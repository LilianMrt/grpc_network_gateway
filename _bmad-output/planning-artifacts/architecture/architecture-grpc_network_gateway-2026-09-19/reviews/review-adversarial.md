---
title: Adversarial review — ARCHITECTURE-SPINE.md (grpc_network_gateway control plane)
lens: adversarial
target: '_bmad-output/planning-artifacts/architecture/architecture-grpc_network_gateway-2026-09-19/ARCHITECTURE-SPINE.md'
against: 'prd.md (FR-1..FR-32, E1-E6) and the repository at 4f5fa3f'
reviewer: adversarial reviewer
date: 2026-09-20
---

# Adversarial review — architecture spine

**Verdict: the spine is coherent on dependency direction and failure taxonomy, and it is
silent on the shared-data shape that E1 and E3 must agree on. Thirteen concrete pairs of
units below each obey every AD to the letter and still build incompatibly; three of them
break the PRD's headline behaviours (FR-16's no-op, UJ-2's demo beat, UJ-3's delete).**

Method: for each finding I name two units one level down — two epics, two stories, or the
Rust builder and the Go builder — show the choice each makes that the ADs permit, show where
they collide, and state the AD that closes the hole. Nothing here is generic advice.

---

## CRITICAL

### F1 — `ListRoutes` has no message. E1 and E3 will invent two different ones.

**Unit A — E1/S-FR-3, the Rust builder.** He is told (AD-1) that `ListRoutes` reads
`vpn_routes` and (AD-4) that `proto/gateway.proto` is the contract. He opens the proto and
finds a ready-made repeated message already in it:

```proto
message RouteDetails {
  string destination_ip = 1;
  string tunnel_id = 2;
  string remote_endpoint = 3;
}
```

Reuse is obviously right by AD-4's "one contract" reasoning, so he ships
`rpc ListRoutes (ListRoutesRequest) returns (StatusResponse)` — or a `ListRoutesResponse`
with `repeated RouteDetails routes`. Either way the key field is named `destination_ip`.
Because `vpn_routes` is the authority and the demo wants to show durable state, he also adds
`created_at` — the column exists, AD-1 says the table is the truth, nothing forbids it.

**Unit B — E3/S-FR-16, the Go builder.** The Consistency Conventions table tells him
"Proto and SQL: `local_ip`, `tunnel_id`, `remote_endpoint`", and Identity tells him "a Tunnel
is keyed on `local_ip` everywhere — CR, proto, table." He writes the one mapping the spine
permits, in `internal/gateway`, against `pb.Route{LocalIp, TunnelId, RemoteEndpoint}`.

**The collision.** Best case, `go build` fails on `LocalIp` and somebody notices. Worst and
likelier case: the Rust builder, reading the same conventions table, adds a *second*
message with `local_ip` alongside `RouteDetails` — and now the repository has two spellings
of a route, `GetGatewayStatus` returning one and `ListRoutes` the other, which is exactly the
drift AD-4 exists to prevent. Note that the conventions table is already *wrong about the
repository as it stands*: `RouteDetails.destination_ip` is committed today and contradicts
"`local_ip` everywhere in proto". The spine asserts a convention the artifact violates and
does not say which one gives way.

**The second collision, and this one is silent.** `created_at` in the response. AD-1 and FR-16
require the operator to decide "already correct" and issue **no write call** when it is —
"verified by call count, not by outcome" (FR-16, and again in FR-24's idempotency test).
Nothing in the spine defines that comparison. The Go builder writes the obvious thing:

```go
if actual == desired { return }   // struct equality over the whole pb message
```

`created_at` never equals anything the CR carries, so every pass is a write, FR-16's
call-count assertion fails, FR-24's idempotency test fails, and SM-3's "idempotency" talking
point is false of the implementation. The builder who hits this will fix it by hand-picking
fields — re-deciding the contract in a story, which is precisely what a spine is for.

FR-13 excludes "any field with no counterpart in `vpn_routes`". The reverse case — a *column*
with no counterpart in the CRD — is the one that will actually be hit, and it is unaddressed.
`vpn_routes` has two such columns today: `id SERIAL` and `created_at`.

**Close it — AD-12 (new): the actual-state record is exactly three fields, and equality is
defined over two.**

> `ListRoutes` returns `repeated Route`, where `Route` is `{ string local_ip = 1; string
> tunnel_id = 2; string remote_endpoint = 3; }` and nothing else. `local_ip` is the field
> name in every proto message that names a tunnel key; `RouteDetails.destination_ip` is
> renamed to `local_ip` in the same commit that adds `ListRoutes`, and `GetGatewayStatus`
> reuses `Route`. Columns of `vpn_routes` outside those three (`id`, `created_at`) are
> invisible to the contract and must never appear in a response message. A tunnel is
> "already correct" iff a `Route` exists with `local_ip == spec.localIP` and both
> `tunnel_id == spec.tunnelID` and `remote_endpoint == spec.remoteEndpoint` compare equal as
> exact byte strings — no trimming, no case folding, no host normalisation. `ListRoutesRequest`
> is empty; there is no per-key filter and no pagination at this scale.

The "exact byte strings" clause is load-bearing and cheap: without it, one builder trims
whitespace on `remoteEndpoint` and the other does not, and the pair oscillates between
"correct" and "needs a write" forever.

---

### F2 — Two `VpnTunnel`s may claim one `localIP`, and the spine's own Identity rule guarantees they fight.

Trace it concretely. `VpnTunnel` is namespace-scoped and named by `metadata.name`, not by
`localIP`. AD-10 permits any namespace in `gatewayRef`. Nothing — no webhook (PRD §8.2
excludes them), no CEL marker (CRD validation cannot see sibling objects) — prevents this:

```yaml
# ns-a/paris                          # ns-b/london
spec:                                 spec:
  gatewayRef: {name: gateway, ns: netgw}  gatewayRef: {name: gateway, ns: netgw}
  localIP: 10.0.0.1                     localIP: 10.0.0.1
  tunnelID: paris                       tunnelID: london
```

**Unit A — the reconcile pass for `ns-a/paris`.** `ListRoutes` shows `10.0.0.1` with
`tunnel_id: london`. Present-but-wrong. FR-16 says one create call, which upserts. It writes
`paris`, sets `Ready/Converged`, advances `observedGeneration`, requeues 30s.

**Unit B — the reconcile pass for `ns-b/london`**, obeying identical rules, writes `london`
back. Both resources report `Ready True / Converged` — honestly, by FR-19's definition —
while the gateway flip-flops forever at 30s. Every pass is a write, so FR-16's no-op property
silently never holds in this configuration, and the log shows two namespaces fighting with no
reason field that explains it.

**The worse branch is deletion.** `kubectl delete vpntunnel -n ns-a paris`. The finalizer path
calls `DeleteVpnTunnel(local_ip: 10.0.0.1)` — which is idempotent and unconditional by FR-2 and
by the committed Rust (`DELETE FROM vpn_routes WHERE local_ip = $1`, no owner predicate). It
deletes `ns-b/london`'s tunnel. `ns-b/london` stays `Ready True / Converged` for up to 30
seconds while its tunnel does not exist. **A resource in one namespace can silently destroy
external state owned by a resource in another namespace, and the victim's status says
everything is fine.** That is not a multi-tenancy nicety; it is the cross-namespace consent
gap the Deferred section waves at, and the spine files it under "correct while single-tenant"
without noticing that the *single-tenant* case hits it too — two manifests in one namespace
with the same `localIP` and different names collide identically.

Worse still, the sequence diagram's delete branch fires `Delete` on *any* pass with
`deletionTimestamp` set, with no check that this operator ever added the finalizer. A
`VpnTunnel` created, never converged, and deleted still issues a delete against a `local_ip`
it never owned.

**Close it — AD-13 (new): a row is owned, and ownership is checked before it is overwritten
or deleted.**

> `vpn_routes` gains an `owner` column (`VARCHAR(253)`, nullable) carrying
> `<namespace>/<name>` of the `VpnTunnel` that created the row; `TunnelRequest` and
> `DeleteTunnelRequest` gain an `owner` field. `CreateVpnTunnel` upserts only when the stored
> owner is NULL or equal to the request's owner, and otherwise returns `FAILED_PRECONDITION`
> naming the current owner. `DeleteVpnTunnel` deletes only a row whose owner matches, and
> returns `success=true, existed=false` otherwise — preserving FR-2's idempotency while making
> the delete unable to reach another resource's tunnel. Go maps `FAILED_PRECONDITION` as
> permanent (AD-3's `INVALID_ARGUMENT` class) onto `Ready False / Conflict`, message naming
> the owner. An empty `owner` means "created out of band" and is freely claimable — which is
> what makes UJ-2 still work.
>
> If the cost is judged too high for MVP, the fallback AD is still an AD and must be written:
> "a duplicate `localIP` across `VpnTunnel`s is unhandled; the operator does not detect it and
> the last writer wins" — recorded in Deferred with an explicit note that **a delete in one
> namespace can remove another namespace's tunnel.** Silence is the one option that does not
> survive a reviewer asking about it, and defending this repo in an interview (SM-3) is the
> whole point of the project.

The `owner` column is also what makes the finalizer safe: delete-by-owner cannot orphan or
over-reach, and the "finalizer added before the first external write" rule (FR-18) becomes
checkable rather than merely asserted.

---

### F3 — Two owners of one table: SM-5 invites a garbage-collector that FR-15 forbids and `make smoke` would be destroyed by.

**Unit A — E4/S-FR-18, the finalizer story.** Deletes exactly the row named by the resource
being reconciled. Obeys FR-15 ("Reconcile is scoped to one resource; it never enumerates or
mutates sibling resources").

**Unit B — E4/S-FR-25 or E5b, the story chasing SM-5** ("across the full test suite, no run
leaves a Tunnel in `vpn_routes` without a corresponding `VpnTunnel`"). The natural reading of
that metric is a sweep: `ListRoutes` already returns every row — AD-1 makes it the authority,
AD-6 says read it fresh every pass — so delete the rows no `VpnTunnel` claims. Every AD
permits this. AD-7 is satisfied (it goes through the API). AD-1 is satisfied. AD-6 is
satisfied.

**The collision is with the product itself.** If Unit B ships, UJ-2's demo becomes a coin
flip: the tunnel Lilian creates by hand through the gateway API to *demonstrate* drift is an
orphan, and the sweep deletes it rather than the resync restoring it. `make smoke` — which
exists today and creates a route directly — becomes destructive under a running operator.
FR-25's integration test, which drives "create → `ListRoutes` → delete → re-delete through
the real gRPC API", races the sweep. And Unit B's sweep violates FR-15's letter, which the
builder will not notice because FR-15 is about *sibling `VpnTunnel`s*, not about rows.

The spine never says who may create a `vpn_routes` row or who may delete one. It says
`vpn_routes` is the authority (AD-1) and that the operator's only channel is gRPC (AD-7) —
neither of which answers "may the operator delete a row it did not create?".

**Close it — AD-14 (new): the operator writes only rows it is reconciling; orphan rows are not
drift.**

> A `vpn_routes` row is created or deleted by the operator only as the direct consequence of
> reconciling the one `VpnTunnel` whose `spec.localIP` equals that row's `local_ip`. The
> operator never deletes a row it did not create, never enumerates rows to reap them, and an
> orphan row — one with no corresponding `VpnTunnel` — is a legitimate steady state, not
> drift, because the gateway API is a public API with other clients (`make smoke`, the UJ-2
> demo, the FR-25 test). `ListRoutes` returning rows the cluster does not declare produces no
> write and no condition. SM-5 is therefore scoped to rows the *test suite's own operator*
> created, and is measured by the finalizer's correctness, not by a sweep.

---

## HIGH

### F4 — Three independent definitions of "valid", and AD-3 turns one permanent failure into an infinite retry.

**Unit A — E2/S-FR-13, the CRD author.** FR-13 says `localIP` is "validated as an IPv4 address
by an API-server-enforced marker" so an invalid value "never reaches the Operator". He writes
`+kubebuilder:validation:Format=ipv4` (or a `Pattern` regex, or CEL `isIP()`), and
`remoteEndpoint` as a `host:port` pattern. He sets no `MaxLength` — nothing tells him to.

**Unit B — E1, the Rust builder.** His validation is `payload.local_ip.parse::<Ipv4Addr>()`,
committed today. And his *storage* validation is the schema: `local_ip VARCHAR(45)`,
`tunnel_id VARCHAR(255)`, `remote_endpoint VARCHAR(255)`.

**Collision 1 — the accept/reject boundary differs.** `Ipv4Addr::from_str` rejects leading
zeros (`010.0.0.1`) and rejects anything non-IPv4; `Format=ipv4` and a hand-written regex do
not agree with it at the edges, and `Format=ip` accepts IPv6 outright. A value that passes
`kubectl apply` and fails in Rust produces `INVALID_ARGUMENT`, which AD-3 classifies as
**permanent, no retry**, onto `Ready False / InvalidSpec`. The user's only remedy is to change
`localIP` — which FR-13 made **immutable**. The resource is permanently wedged, and
delete-and-recreate is the only path out, with nothing in the status saying so.

**Collision 2, and this is the sharper one — AD-3 misclassifies a permanent failure as
retryable.** `tunnelID: <300 chars>` passes the CRD (no `MaxLength`) and hits Postgres
`22001 string_data_right_truncation`. AD-3's rule: "constraint or unexpected error to
`INTERNAL`". Go's rule: "`UNAVAILABLE` and `INTERNAL` return the error and requeue under
`controller-runtime` backoff". So a permanently-bad input retries forever under exponential
backoff, burning a worker slot and filling the log, with `Ready` never settling on a reason a
user can act on. FR-20's "not a tight loop and not a wedged worker" is honoured in the letter
and violated in spirit. The same applies to any future `CHECK` or `NOT NULL` violation:
**AD-3's taxonomy has no category for "the caller's data is bad in a way only the database can
see", and that category is certain to be hit.**

`remote_endpoint` has a third asymmetry: the CRD validates it as `host:port`, and the Rust
side validates it **not at all** — it is stored as an arbitrary string. So the CRD is the only
guard on a field the gateway will happily accept garbage into from `make smoke`.

**Close it — AD-15 (new): one validation authority per field, mirrored, with a length budget;
and a permanent-data class in AD-3.**

> The gateway is the authority on validity; the CRD markers are a *mirror* of the gateway's
> rules whose only job is to fail fast at `kubectl apply`. They are listed together in one
> place and must be kept in lockstep: `localIP` — a dotted-quad IPv4 address as
> `std::net::Ipv4Addr` parses it, mirrored by a CEL/regex marker rejecting leading zeros;
> `tunnelID` — 1..=255 characters, mirrored by `MaxLength=255`; `remoteEndpoint` — `host:port`,
> 1..=255 characters, **validated in Rust before any write** (currently it is not) and mirrored
> by `MaxLength=255` plus the pattern. Any marker the gateway does not also enforce, or any
> gateway rule not mirrored in a marker, is a defect.
>
> AD-3 is amended: a database error identifying bad caller data — Postgres SQLSTATE class `22`
> (data exception) and `23514` (check violation) — maps to `INVALID_ARGUMENT`, not `INTERNAL`,
> because it is permanent and retrying it is futile. `23505` (unique violation) remains
> `INTERNAL`; it is a bug in the upsert, not a bad request. Go's `INVALID_ARGUMENT` branch
> additionally states in the `Ready` message that the spec must be corrected, and — because
> `localIP` is immutable — names delete-and-recreate as the remedy when the offending field is
> `localIP`.

---

### F5 — `AD-10` builds an address with a `<port>` that no unit owns.

AD-10's rule is literal: "`gatewayRef` carries name and namespace, and both are used verbatim
to build `<name>.<namespace>.svc.cluster.local:<port>`."

**Unit A — E2/S-FR-13, the CRD author.** FR-13 says `spec` carries "`gatewayRef` (name +
namespace)". He writes exactly that:

```go
type GatewayRef struct {
    Name      string `json:"name"`
    Namespace string `json:"namespace"`
}
```

**Unit B — E3/S-FR-21, the dialler in `internal/gateway`.** He needs a port and has no field
to read it from. He either hardcodes `50051`, or reads an operator flag, or adds
`Port *int32` to `GatewayRef` — reopening the CRD in a different epic, after E2's story is
closed, and changing an API type that E2 owns.

**The collision is three-way with the chart.** If Unit B hardcodes, AD-11's "every published
port binds 127.0.0.1" and `k8s/30-gateway.yaml`'s `port: 50051` become an uncommitted
coupling; if the chart later makes the Service port a Helm value (FR-12 says "image tag and
replica count are values" and says nothing about the port), the operator silently dials the
wrong port and every resource reports `GatewayUnreachable` with no indication why. If Unit B
adds `Port`, AD-6's "connections are cached per `gatewayRef`" now has two possible cache keys —
the struct including port, or the name/namespace pair — and two entries can resolve to one
address or one entry to two.

**Close it — AD-16 (new): the gateway port is a fixed constant of this system, not a field.**

> The gateway listens on 50051 everywhere: `BIND_ADDR`'s default, the Service port, the
> NetworkPolicy, and the address `internal/gateway` builds. `gatewayRef` carries name and
> namespace only, per FR-13; the port is a package constant in `internal/gateway`, the one
> place AD-10 already designates as the home of the naming mapping, and the chart does not
> expose it as a value. The connection cache in AD-6 is keyed on the fully resolved address
> string, so cache key and dial target cannot diverge.

---

### F6 — AD-4 is unsatisfiable for the only commit that actually changes the proto, and `.sqlx/` has the same problem with no rule at all.

**Unit A — E1, Phase A.** E1 is the epic that adds `ListRoutes` to `proto/gateway.proto`. AD-4:
"Any edit to a `.proto` file regenerates both in the same commit", into a committed
`operator/internal/gatewaypb/` via `make proto`.

**Unit B — E2/E3, Phase B.** The operator module does not exist yet in Phase A. There is no
`operator/` directory, no `go.mod`, and — per the spine's own closing line — **Go is not
installed on this host**. So the one commit AD-4 was written for is the one commit that cannot
obey it. The E1 builder will either violate AD-4 knowingly (and the rule loses its force for
every later edit) or block Phase A on installing Go, contradicting PRD §9's "E1 depends on
nothing" and its Phase-A-is-a-weekend budget.

**The unnoticed sibling.** `.sqlx/` is generated build metadata committed for exactly the same
reason as `gatewaypb/` — so the container image builds with no database (FR-9,
`SQLX_OFFLINE=true`). `ListRoutes` adds a new `sqlx::query!`, which requires
`make db-up && make migrate && make prepare` and a regenerated `.sqlx/` in the same commit.
AGENTS.md says so; **the spine does not.** A builder who obeys every AD and skips `make prepare`
ships a repository where `make check` passes locally against a live DB and `docker build`
fails — breaking FR-9 and SM-2's cold-start quickstart. AD-4 governs one generated artifact and
is silent on the other.

**Close it — AD-4 amended, covering both generated artifacts and naming the Phase A exception.**

> Every generated artifact committed to this repository — `operator/internal/gatewaypb/` and
> `.sqlx/` — is regenerated in the same commit as the source it derives from
> (`proto/*.proto`, any `sqlx::query!`). **Exception, stated once:** while `operator/` does not
> exist, a `.proto` edit carries no Go regeneration; the first commit that scaffolds the
> operator generates stubs for the proto as it then stands, and the rule binds from that commit
> onward. `.sqlx/` has no exception and binds from E1's first query. A `make verify` target
> runs `make proto && make prepare` and fails if either leaves the working tree dirty — that
> check, not the prose, is what makes AD-4 enforceable.

---

### F7 — Helm never upgrades a CRD in `crds/`, so AD-8 and FR-23 hold only on the first install.

**Unit A — E5a/S-FR-12, the chart author.** AD-8: "the `VpnTunnel` CRD ships in `crds/` so it
installs first." FR-12 and FR-23 agree, and FR-23's testable consequence is "after `helm
install`, the CRD is present before the Operator pod starts." All true — on a *fresh install*.

**Unit B — E2/S-FR-14, the printer-columns story, or any later CRD edit.** He adds
`+kubebuilder:printcolumn` for local IP / tunnel ID / Ready / age, regenerates the CRD, and
runs `make deploy`. AD-8 defines `make deploy` as `helm upgrade --install`. **Helm's `crds/`
directory is installed on `install` and ignored on `upgrade` — by design, with no flag to
change it.** The CRD in the cluster is the old one. `kubectl get vpntunnels` shows no printer
columns and the story looks broken; worse, if the edit added a required field or a validation,
the operator serves a type the API server will not accept, and the failure surfaces as
admission errors the builder will chase into the controller.

This is not a hypothetical footgun; it is the single most common Helm-CRD defect, and the
spine's rule points straight at it.

**Close it — AD-17 (new): the CRD is applied by the Makefile, before Helm runs.**

> `make deploy` applies the CRD explicitly (`kubectl apply --server-side -f
> charts/netgw/crds/`) and only then runs `helm upgrade --install`. The CRD stays in `crds/`
> so a bare `helm install` also works, but the Makefile — which AD-8 already declares the
> interface — is what guarantees the CRD is current on every deploy, not only the first.
> `helm uninstall` leaving the CRD behind is intended and is stated in the README: FR-12's
> "leaves no namespaced objects behind" is satisfied because a CRD is cluster-scoped.

---

## MEDIUM

### F8 — The delete branch of the reconcile diagram contradicts FR-18, and "Ready during Deleting" is undefined.

**Unit A — E4/S-FR-18, reading the sequence diagram.** The diagram's `deletionTimestamp`
branch is three steps: `Delete by localIP` → `DeleteVpnTunnel` → `remove finalizer`. No status
write, no error branch. He implements exactly that.

**Unit B — E4/S-FR-18, reading the FR.** FR-18: "While the Gateway is unreachable, the
resource remains in `Terminating` with an explanatory condition, and completes once the
Gateway returns." The conventions table lists a `Deleting` reason, which only exists to be
written on this path.

They differ on whether the delete path writes status at all, and the diagram — the spine's own
normative picture, labelled "the behaviour every story in E3 and E4 is measured against" — is
the one that is wrong. Three further cases neither unit can resolve from the spine:

- **`Ready`'s status during `Deleting`** — `False/Deleting`, or left `True` until the tunnel is
  actually gone? Two builders will choose differently and UJ-3's `kubectl describe` output
  differs accordingly.
- **Ordering of the status write and the finalizer removal.** Status first then finalizer is
  correct; finalizer first then status yields a `NotFound` on the status update (the object is
  collected the instant the last finalizer goes), an error return, a requeue, and a log line
  that looks like a bug forever.
- **`observedGeneration` on the delete path** — advanced or frozen? FR-19 ties it to "a
  successful converge", and a successful *deletion* is not obviously one.
- The diagram's final `requeue after 30s` sits outside the `alt`, so it applies to the delete
  branch too: a requeue scheduled for an object that no longer exists. Harmless under FR-15's
  clean no-op, but it is an instruction to do a pointless thing, and a builder will implement it.

**Close it — extend the Conditions convention, and fix the diagram.**

> On a pass with `deletionTimestamp` set: write `Ready False / reason=Deleting` **before**
> calling the gateway; on `DeleteVpnTunnel` success remove the finalizer and return with no
> requeue and no further status write; on failure leave the finalizer, write
> `Ready False / reason=GatewayUnreachable` with the gRPC code in the message, and return the
> error so `controller-runtime` backs off. `observedGeneration` is not advanced on the delete
> path. The sequence diagram is corrected to show the status write and the failure branch, and
> the requeue moves inside the non-deletion branch.

### F9 — AD-2 and AD-9 assign E1 work that no FR carries, so no story will own it.

AD-2: "no `sqlx::query!` appears outside `src/store/`." AD-9: "no `println!` survives E1."
The repository at 4f5fa3f has **no `src/store/` at all** — all three queries and `hydrate()`
live in `src/services/gateway.rs`, and there are seven `println!` calls in that one file.

**Unit A — the story generator working from PRD §9.** E1 is "FR-3, FR-31". FR-3 adds an RPC;
FR-31 swaps two statements' order. Neither mentions extracting a store module, moving
`hydrate()`, or replacing `println!` with `tracing`. The stories generated will be small.

**Unit B — anyone later reading AD-2.** The rule is stated in the present tense as though it
describes the code. It does not; it describes a target. The first builder to add an RPC after
E1 will find `sqlx::query!` in `services/` and reasonably conclude the rule is aspirational.

This is not a data incompatibility, it is a scope hole that produces one: AD-2's whole purpose
("write ordering being re-decided by whoever adds the next RPC") fails if the refactor never
gets a story.

**Close it — AD-2 amended with an explicit landing point.**

> AD-2 and AD-9 are *not yet true of the repository*; E1 is the epic that makes them true.
> E1's scope includes: extracting `src/store/mod.rs` with every `sqlx::query!` and the
> `hydrate` query, reducing `services/gateway.rs` to validate → store → cache, and replacing
> every `println!` with `tracing`. E1 is not complete while `rg 'sqlx::query!' src/ --glob
> '!src/store/**'` or `rg 'println!' src/` returns a hit. Stating the grep makes it a
> testable consequence rather than a wish.

### F10 — AD-6's connection cache is one sentence short of two incompatible dial strategies.

**Unit A — E3/S-FR-21.** "Every call carries a timeout" (FR-21, AD-5). He uses
`grpc.Dial(addr, grpc.WithBlock(), grpc.WithTimeout(...))` — still the shape most examples
show — and puts the per-call timeout on the context. After a gateway pod restart, the cached
`ClientConn` is in `TRANSIENT_FAILURE`; the blocking call consumes the whole per-call timeout
before returning, the worker stalls for that duration per resource, and with several resources
the 30s resync budget is gone. `grpc.Dial` is also deprecated in grpc-go v1.84.

**Unit B — E5b or the FR-25 test author.** He uses `grpc.NewClient`, which is lazy and
non-blocking, and gets a fast `Unavailable` that AD-3 maps cleanly onto `GatewayUnreachable`.

Both obey AD-6 ("connections are cached per `gatewayRef` for the process lifetime and
reused"). The observable behaviour under gateway restart — the exact scenario UJ-1's edge case
and FR-18's "completes once the Gateway returns" describe — differs entirely. The spine also
says nothing about the cache's key (see F5), its eviction (never — a `gatewayRef` typo leaks a
`ClientConn` for the process lifetime), or whether a connection is ever closed.

**Close it — AD-6 amended.**

> `internal/gateway` holds a mutex-guarded `map[string]*grpc.ClientConn` keyed on the resolved
> address. Connections are created with `grpc.NewClient` — never `grpc.Dial`, never
> `WithBlock` — so dialling is lazy and a dead gateway fails fast as `UNAVAILABLE` rather than
> consuming the FR-21 call timeout. Entries are never evicted and are closed only at process
> shutdown. The per-call timeout is a `context.WithTimeout` on each RPC, defaulting to 5s and
> exposed as an operator flag and a chart value.

---

## LOW

### F11 — `TunnelResponse.success` and `DeleteTunnelResponse.existed` have no stated meaning for the operator.

`CreateVpnTunnel` returns `success: true` unconditionally; every real failure arrives as a
`Status`. A Go builder who writes `if !resp.Success { return err }` writes dead code; one who
uses `existed == false` to decide anything contradicts FR-2's whole point. AD-3 says "an error
is never rendered as an empty result" but never says the converse — that the gRPC status is the
*only* success signal.

**Close it — one line in the Errors convention:** the gRPC status code is the sole success
signal on every RPC. `success` is informational and must not be branched on; `existed` is for
logging and for the FR-25 assertion only, never for control flow.

### F12 — Q3 is marked "flag for `bmad-architecture`" and the spine answers it only by implication.

PRD §11 Q3 ("does `ListRoutes` read `vpn_routes` directly, or force a re-hydration?") is
explicitly routed to this document. The spine answers it in two places that a builder must
join up: AD-1's "every control-plane read comes from the store", and Deferred's "gateway-owned
periodic re-hydration ... deliberately not folded into `ListRoutes`". That is an answer, but it
is not *in the Invariants section*, and a builder reading only AD-1 could still argue that
re-hydrating and then reading the store satisfies it. Add one clause to AD-1: "`ListRoutes`
performs a direct `SELECT` and never triggers hydration as a side effect; a pod that has never
hydrated still serves `ListRoutes` correctly (FR-3)." Note the readiness interaction worth
stating alongside it: readiness gates on hydration (FR-8), so a pod that *can* answer
`ListRoutes` is out of the Service endpoints until hydration succeeds — correct, but
non-obvious, and a builder debugging a `GatewayUnreachable` at startup will chase it.

### F13 — The spine is not in the repository it governs.

`.gitignore` on this branch adds `_bmad-output`, so `ARCHITECTURE-SPINE.md` — the document
every AD above amends — is untracked. FR-29 puts ADRs in `docs/adr/` and FR-28 makes the repo
legible to a stranger in four minutes; the governing architecture is currently invisible to
both. Either commit the spine (or a distilled `docs/adr/` set derived from it) or state
deliberately that the planning artifacts are local-only and the ADRs are the public record.

---

## What is solid, and why it matters to the above

Three ADs do real load-bearing work and should not be touched while closing the holes:

- **AD-7** (one-way dependency) is the sharpest rule in the document. The prohibition diagram
  with dotted arrows makes "the operator must not read Postgres" checkable rather than
  exhortative, and it is what keeps the operator an honest client of the API it exists to prove.
- **AD-5** (`internal/controller` must not import `google.golang.org/grpc`) is testable by a
  grep and single-handedly makes FR-16's call-count assertion possible. Keep the import ban
  literal.
- **AD-1's** separation of the three state locations, and the explicit "`GetGatewayStatus` must
  never be substituted for `ListRoutes`", correctly encode the PRD's most important
  `[NOTE FOR PM]`.

The pattern across the findings is narrow and consistent: **the spine is strong on *direction*
— who may call whom, what may not import what — and silent on *shape* — what exactly crosses
the wire, what exactly counts as equal, who exactly owns a row.** AD-12, AD-13, AD-14 and AD-15
are the four that close that gap; the rest are ordering and lifecycle tightenings.

## Suggested AD numbering, for the update

| New / amended | Closes | Section |
| --- | --- | --- |
| **AD-12** `ListRoutes` shape and the equality rule | F1 | Invariants |
| **AD-13** Row ownership and owner-checked delete | F2 | Invariants |
| **AD-14** Orphan rows are not drift; no sweep | F3 | Invariants |
| **AD-15** One validation authority, mirrored; permanent-data error class | F4 | Invariants + AD-3 |
| **AD-16** The gateway port is a constant, not a field | F5 | Invariants (or fold into AD-10) |
| **AD-4 amended** all generated artifacts co-commit; Phase A exception; `make verify` | F6 | AD-4 |
| **AD-17** The Makefile applies the CRD before Helm | F7 | Invariants (or fold into AD-8) |
| **Conditions convention extended** delete-path status, ordering, `observedGeneration` | F8 | Conventions + diagram |
| **AD-2 amended** E1 owns the `store/` extraction and the `println!` removal | F9 | AD-2 / AD-9 |
| **AD-6 amended** cache key, `NewClient`, no `WithBlock`, timeout default | F10, F5 | AD-6 |
| **Errors convention** the gRPC status is the sole success signal | F11 | Conventions |
| **AD-1 clause** `ListRoutes` is a direct read, never hydrates | F12 | AD-1 |
