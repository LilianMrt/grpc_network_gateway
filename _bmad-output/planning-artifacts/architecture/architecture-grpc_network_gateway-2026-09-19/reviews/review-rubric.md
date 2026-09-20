---
title: Rubric review — ARCHITECTURE-SPINE.md (grpc_network_gateway control plane)
reviewer: rubric reviewer (architecture spine)
target: ARCHITECTURE-SPINE.md, updated 2026-09-20
evidence: repo at 4f5fa3f + working tree, PRD + addendum, .memlog.md, live host inspection 2026-09-20
date: 2026-09-20
---

# Rubric review — Architecture Spine

## Verdict

**Sound spine, not yet shippable as written.** AD-1 through AD-7 are the real thing: they name the
actual divergence points of a level-triggered loop split across two languages, and most of their
rules are mechanically checkable. Coverage of the PRD is complete (all 32 FRs appear in the
capability map), three of five PRD open questions are closed with reasons, and terseness is held.

The failures cluster in two places, and both are structural rather than verbosity problems:

1. **The spine carries no record of where it overrides its own inputs.** It deliberately contradicts
   PRD §6, AGENTS.md (twice), and the documented semantics of two *delivered* FRs. Every one of
   those overrides is in `.memlog.md`; **none is in the spine**. The memlog is a working log, not a
   contract — the epic generator and every agent session reads the spine and AGENTS.md, not the
   memlog.
2. **The operator's deployment/identity envelope is underspecified** in four specific ways that E2,
   E3 and E5b will each answer independently: watch scope, RBAC scope, operator replica count, and
   where the gateway's *port* comes from.

Both are fixable with roughly fifteen lines. Nothing below asks for more prose, more sections, or
more detail for its own sake; three of the recommendations *shorten or tighten* existing text.

---

## 1. Does it fix the real divergence points for E1–E6? Does it miss any?

### What it gets right

| Divergence point | Fixed by | Assessment |
| --- | --- | --- |
| Which state is authoritative | AD-1 + the three-state table | Excellent. The table is doing more work than the prose and is the single best artifact in the document. |
| Write ordering re-decided per RPC | AD-2 | Right target — FR-31 exists *because* this was never a rule. |
| One failure, two status codes | AD-3 | Right target; the Rust/Go split of responsibility is clean. |
| Two programs drifting on one `.proto` | AD-4 | Right target, and keeping `protoc` off the `go test` path is the correct second-order call. |
| gRPC leaking into `Reconcile` | AD-5 | Strongest rule in the document (see §2). |
| Edge-triggered creep / cached actual state | AD-6 | Right target; correctly separates transport from state. |
| Operator shortcutting to Postgres | AD-7 | Right target; the prohibition diagram earns its place. |
| Two deployment paths | AD-8 | Closes PRD Q5. |
| Registry / cluster / port assumptions | AD-11 | The operational envelope is **not** silent — see §7. Its factual premise is wrong, though; see §4. |

### Misses

**M1 — Where does the gateway's *port* come from? (high)**
AD-10's Rule says `gatewayRef` "carries name and namespace, and both are used verbatim to build
`<name>.<namespace>.svc.cluster.local:<port>`." `<port>` appears in the format string and is defined
nowhere. PRD FR-13 fixes `spec.gatewayRef` as name + namespace only. So E2 (CRD author) and E3
(client author) each face an unowned choice: a third `gatewayRef.port` field, an operator flag, or a
hardcoded 50051. This is precisely the class of thing the spine exists to remove, and it is one
clause long. **Decide it inline in AD-10** (recommend: constant 50051, matching the Service port in
`k8s/30-gateway.yaml` — an operator flag would be a second way to say the same thing).

**M2 — Is the operator's manager namespace-scoped or cluster-scoped? (high)**
Unstated anywhere. It determines: whether FR-22 generates a `Role`+`RoleBinding` or a
`ClusterRole`+`ClusterRoleBinding`; whether the manager sets `Cache.DefaultNamespaces`; and whether a
`VpnTunnel` in a namespace other than the operator's is reconciled at all. That last consequence
directly contradicts AD-10's "Any namespace is permitted" if E5b happens to scope the cache — AD-10
permits a cross-namespace *target*, which is a different axis from a cross-namespace *watch*, and
the spine conflates them by leaving one silent. E3 (manager setup) and E5b (chart RBAC) will answer
this separately and one of them will be wrong.

**M3 — Operator replica count is never pinned, while leader election is deferred. (high)**
AD-11 pins `replicas: 1` for the gateway and says nothing about the operator. Deferred lists "leader
election … none affects a decision taken here." That is only true while the operator runs one
replica. Two operator replicas without leader election is two reconcilers racing writes against the
same gateway — a correctness bug, not a scaling nicety. **Add "and so does the operator" to AD-11's
replica sentence**; it costs five words and makes the deferral safe. (Cross-listed under §3.)

**M4 — The schema exists in two hand-maintained copies. (medium)**
`migrations/01_init_routing_table.sql` and the inline literal in `k8s/12-configmap-initdb.yaml` are
the same DDL, duplicated, with nothing keeping them in step. The Conventions "Schema" row settles
*mechanism* (init hooks, not `sqlx migrate`) but not *source*. E5a turns the ConfigMap into a chart
template and will either template the file in or re-copy the literal. One clause: "`migrations/` is
the single source; the chart's initdb ConfigMap is generated from it via `.Files.Get`."

**M5 — Nothing states what `make deploy` does about the image tag. (medium)**
AGENTS.md records the trap in full: with a fixed `:dev` tag and `imagePullPolicy: IfNotPresent`,
`make deploy` re-applies an unchanged manifest and the pod keeps the old binary — you need
`make load` then `kubectl rollout restart`. AD-8 replaces `make deploy` with `helm upgrade --install`
and inherits the identical trap (a Helm upgrade with an unchanged image value produces no new pod
template hash, so no rollout). FR-12 makes the image tag a chart value, which is the escape hatch,
but the spine does not say to use it. This will cost a confused hour in E5a/E5b and again in E6 when
the quickstart is verified.

**M6 — No Rust-side verification story at all. (low–medium, deliberate but unstated)**
AGENTS.md: "There is no test suite and no CI; `make check` is the only automated verification." E1
extracts `src/store/`, reorders the create path, and changes the error taxonomy on two *delivered*
RPCs — with zero Rust tests and the only regression coverage (FR-31) deferred to a Go integration
test in E4. That may well be the right trade at this scale, but it is a decision, and the spine
records it neither as a decision nor as an accepted risk. One line under Deferred would do it.

---

## 2. Is every AD's Rule enforceable, and does its "Prevents" follow?

### Enforceable and load-bearing

- **AD-5** — `operator/internal/controller` must not import `google.golang.org/grpc`. Greppable,
  and testable as a one-line import check. Its Prevents follows exactly: without the boundary,
  FR-16's "verified by call count" has nothing to count against. Best rule in the document.
- **AD-7** — greppable in `go.mod` (no `pgx`/`database/sql`). Prevents follows.
- **AD-4** — "`go build` and `go test` must never require `protoc`" is directly testable on a clean
  machine. Prevents follows. See §3 for the missing staleness gate.
- **AD-9** — "no `println!` survives E1" is greppable (7 call sites today: 4 in `gateway.rs`, 2 in
  `health.rs`, 1 in `main.rs`). Prevents follows.
- **AD-11** — every clause is a checkable property of `kind/cluster.yaml`, the Dockerfile and the pod
  specs. Prevents follows. (Its factual premise is false; §4.)

### Rules that are weaker than they read

**F1 — AD-2's rule is written against the wrong token, and the one SQL statement outside the store
today slips through it. (high)**
The Rule says: *"no `sqlx::query!` appears outside `src/store/`."* But `src/services/health.rs:59`
runs `sqlx::query("SELECT 1").execute(&pool)` — the **function** form, not the macro. It is a
database read, it is outside `src/store/`, and AD-2 as written permits it forever. The readiness
ping is *exactly* the kind of "small, obviously fine" direct query that erodes a store boundary, and
it is already in the tree. Fix: say **"no SQL — `sqlx::query`, `sqlx::query!`, or any other form —
appears outside `src/store/`"**, and move the ping to `store::ping()`. Same length, and now the rule
covers what it claims.

**F2 — AD-2's ordering half is a preference dressed as a rule. (medium)**
"A handler may call the store and then the cache; never the reverse" is review-enforced only.
Nothing structural stops `router.add_route()` preceding `store::create()` — which is precisely the
FR-31 defect the AD is named for. The first half of AD-2 is greppable; the second half is not, and
they are presented at identical strength. Cheapest structural fix: make `RoutingTable`'s mutators
`pub(crate)` and reachable from one place, or have `store::create_route` return the value the cache
is fed, so the ordering is enforced by data flow rather than by discipline. Either way the spine
should say which, or say plainly that this half is review-enforced.

**F3 — AD-10's Prevents contains a closed RBAC list that collides with FR-27. (medium–high)**
"…so its RBAC covers `vpntunnels`, `vpntunnels/status` and events only." Current
`controller-runtime`/kubebuilder scaffolds serve metrics (FR-27) from an authenticated, authorized
endpoint, which requires `create` on `authentication.k8s.io/tokenreviews` and
`authorization.k8s.io/subjectaccessreviews`. An E5b author who enables FR-27 as scaffolded must
either violate AD-10's "only" or silently disable metrics auth. AD-10's real content — *no reads of
Service objects* — is correct and worth keeping; the closed enumeration is a bonus claim it cannot
cash. Narrow the clause to the claim the rule actually supports. (The PRD's FR-22 makes the same
"and nothing else" claim, so this is inherited, not invented — which is exactly when a spine should
catch it.)

**F4 — AD-6's second "Prevents" does not follow from FR-32. (low)**
It claims to prevent "both failure modes of FR-32 — caching observed routes between passes, and the
over-reading that redials a fresh connection every resync." FR-32 says retain nothing about the
gateway between passes; redialing each pass *complies* with FR-32 and is merely wasteful. The
connection-caching clause is a good decision, but it is an independent one, not a second failure
mode of FR-32. Reword or drop the claim; the rule stands on its own.

**F5 — AD-3 is correct, but its cost is understated. (medium — see §5)**
The taxonomy is enforceable at the store boundary. What the Prevents line does not say is that
adopting it **changes behaviour the PRD documents as a tested, delivered consequence** of FR-1
("A database failure returns `INTERNAL`") and FR-2. That is a deliberate, defensible override — and
it needs to be visible to whoever generates stories for E1, who will otherwise read FR-1 as
`Delivered — no story` and leave it alone.

---

## 3. Could anything under Deferred still let two units diverge right now?

Three can.

**D1 — Leader election. (high, = M3)** Safe only if the operator runs one replica. Nothing in the
spine pins that.

**D2 — "Migration Job instead of init-hook seeding — *Revisit while building the chart.*" (medium)**
This is not deferred past the current work; it is deferred *into* E5a, which is Phase A and
scheduled now. Its two answers produce materially different charts (templated initdb ConfigMap vs a
migration Job plus `sqlx migrate` in the gateway image plus `_sqlx_migrations` bookkeeping). The
Conventions "Schema" row does decide the mechanism for now — good — but the Deferred entry reopens
it in the same breath, inside the epic that is about to build it. Either close it (recommend: keep
init hooks for MVP, one clause) or move the revisit point to after E5a. As written, the E5a author
has standing permission to pick either.

**D3 — CI. (medium)** With CI deferred there is no automated gate on the three rules whose whole
value is that they are mechanically checkable: AD-2's grep, AD-4's "both generations in one commit",
and AD-9's "no `println!`". AD-4's staleness case is the sharpest — regenerating Go stubs is a
manual `make proto`, and a `.proto` edit committed without it produces exactly the silent drift AD-4
exists to prevent. This does not require CI: `make proto && git diff --exit-code operator/internal/gatewaypb`
folded into `make check` is a local gate, costs one line, and makes AD-4 self-enforcing.

Safe as deferred: multi-replica gateway and re-hydration (AD-11 pins `replicas: 1`); cache-vs-durable
divergence (no unit would invent that condition); cross-namespace consent (AD-10 decides
permissively and says so); webhooks; grouping CRD. The §7 exclusions are correctly labelled
*excluded, not deferred* — a distinction the spine is right to preserve.

One low note: when `k8s/40-networkpolicy.yaml` becomes a chart template, the operator pod gains no
egress rule to the gateway. Unenforced on kind and explicitly a non-goal, so not a finding — but the
templates should not be copied forward as if they described the running topology.

---

## 4. Is named technology verified-current rather than asserted?

**Largely yes, and unusually well.** The memlog carries three dated verification entries against live
sources, not training data, and I re-confirmed every host-checkable claim on 2026-09-20:

| Claim | Host check | Result |
| --- | --- | --- |
| Rust 1.97.1 installed | `rustc --version` | 1.97.1 (8bab26f4f 2026-07-14) ✓ |
| sqlx-cli matches sqlx 0.9.0 | `sqlx --version` | sqlx-cli 0.9.0 ✓ |
| kind v0.33.0 | `kind --version` | 0.33.0 ✓ |
| kubectl v1.37.0 | `kubectl version --client` | v1.37.0 ✓ |
| Helm v4.3.0 | `helm version --short` | v4.3.0+gbec5b06 ✓ |
| Go absent (blocker) | `go version` | command not found ✓ |
| tonic / tonic-prost / tonic-health 0.14.6 | `Cargo.lock` | 0.14.6 ✓ |

Three defects, all small:

- **V1 (low)** — the Stack row "tonic · prost · tonic-prost · tonic-health | 0.14.6" is wrong for
  `prost`: `Cargo.lock` resolves **prost 0.14.4**. `prost` versions independently of the tonic
  family; grouping it into one row asserts a version the repo does not have.
- **V2 (low)** — `tracing-subscriber 0.3` is listed in Stack, but it is **not in `Cargo.toml`**
  (only `tracing 0.1` is, and it is unused — zero references in `src/`). The Stack table is meant to
  be a seed of what is true; this row is a requirement of AD-9 masquerading as an observation. Fine
  either way, but worth knowing it is an addition E1 must make, not a fact.
- **V3 (low)** — "kindest/node v1.37.0, pinned **by digest**" names no digest, and
  `kind/cluster.yaml` still carries `image: kindest/node:v1.33.1` with no digest. Only one file is
  affected so divergence risk is nil, but the rule is not yet satisfiable as stated.

**And one that is not small — see the next section.**

---

## 5. Does it ratify the brownfield, and are its deliberate contradictions flagged?

Ratification is good. The spine keeps, without re-litigating: the liveness/readiness split and its
rationale; `connect_lazy`; wholesale-replace hydration; persist-before-cache on delete; `replicas: 1`
and no PDB; committed dev credentials; `IfNotPresent` and no registry; loopback-bound ports; the
FR-11 posture; `OUT_DIR` proto generation; the lib/bin split; the Makefile as the human interface.
That is exactly the right instinct for a brownfield spine, and the Conventions table is where most
of the ratification lives.

**The contradictions are the problem — all four are in `.memlog.md` and none is in the spine.**

**C1 — AD-11 asserts as settled fact a host change that has not happened. (critical)**
AD-11's Rule reads: *"node image pinned by digest at v1.37.0 — the host runs the unified cgroup
hierarchy (`kernelCommandLine = cgroup_no_v1=all` in `.wslconfig`), so the 1.33 pin is retired."*

Verified on this host, 2026-09-20:

```
tmpfs on /sys/fs/cgroup type tmpfs (ro,nosuid,nodev,noexec,mode=755)
cgroup on /sys/fs/cgroup/memory type cgroup (rw,...,memory)
cgroup on /sys/fs/cgroup/pids   type cgroup (rw,...,pids)
   ... 14 further v1 controllers ...
$ cat /mnt/c/Users/lilia/.wslconfig
cat: /mnt/c/Users/lilia/.wslconfig: No such file or directory
```

The host still mounts the hybrid v1 hierarchy and `.wslconfig` does not exist. The memlog states
this correctly as an **action** ("Add `kernelCommandLine` … then `wsl --shutdown`, **then** pin by
digest"); the spine states it as a **condition**. Consequence: anyone following AD-11 today pins
v1.37.0 and `make cluster-up` dies at `error execution phase wait-control-plane: context deadline
exceeded` — the exact failure the addendum §C documents as costing a full session, and which does
not name its own cause. Worse, the prerequisite is owned by no epic: it is a host mutation outside
the repository entirely, and the spine's closing paragraph names the *Go* toolchain as the
un-owned prerequisite while this one goes unlisted beside it.

Fix: state it as a prerequisite in the same breath as Go, and keep AD-11's rule conditional on it
("once the host runs the unified hierarchy; until then the v1.33.1 pin stands and `kind`'s prebuilt
floor forces v1.35.8"). The memlog already contains the fallback analysis; the spine dropped it.

**C2 — AD-9 contradicts AGENTS.md, which every agent session reads. (high)**
AGENTS.md, inside the managed `bmad:context` block, says: *"Use the `log` crate for logging — never
`tracing` (the `Cargo.toml` dependency is unused), and no new `println!` calls."* All three clauses
are wrong (`log` is not a dependency; `tracing` is the only declared logging dep; every shipped log
line is a `println!`), and AD-9 mandates the opposite of two of them. The memlog flags this and
schedules a `bmad-project-context` refresh after E1. **The spine says nothing.** Until the refresh
lands, an agent implementing E1 has two governing documents in direct conflict, and AGENTS.md is the
one loaded automatically.

**C3 — AD-3 changes the documented semantics of two `Delivered — no story` FRs. (medium)**
PRD FR-1 lists "A database failure returns `INTERNAL`" as a *testable consequence of delivered work*;
AD-3 makes connectivity failures `UNAVAILABLE` on every RPC including FR-1 and FR-2. The memlog
accepts the cost explicitly and folds it into E1. The spine does not say so, and PRD §0 makes
`Status:` binding on downstream workflows — so the epic generator is being told "do not generate a
story" for a handler the spine requires changing.

**C4 — AD-11 overrides PRD §6, which deliberately deferred the cgroup fix. (medium)**
PRD §6: *"A permanent host-side fix exists … and is deliberately deferred."* AD-11 un-defers it. That
is the right call, and the memlog says so in as many words ("OVERRIDES PRD section 6 … and
AGENTS.md's pitfall … both need updating"). The spine presents it as though no prior decision
existed.

**C5 — "A Tunnel is keyed on `local_ip` everywhere — CR, proto, table" is false of the shipped
proto. (high)**
`proto/gateway.proto` spells it **`RouteDetails.destination_ip`** in `StatusResponse`. So the
Identity convention — the row whose entire job is to stop one concept acquiring three spellings —
asserts a uniformity the contract file does not have, and does not flag the exception. This is not
cosmetic: E1 must add `ListRoutes`, and its response shape is undecided. Reusing `RouteDetails`
carries `destination_ip` into the operator's actual-state read and breaks the convention at the one
place it matters most; introducing a parallel message with `local_ip` puts two spellings of the key
in one 60-line `.proto`. Decide it in the spine (recommend: a new `Route` message using `local_ip`,
with `RouteDetails` left alone as the legacy debug shape, and a line in Conventions naming
`destination_ip` as a known exception confined to `GetGatewayStatus`).

**Recommendation (covers C1–C5, ~10 lines, no new prose elsewhere):** add a short
**"Overrides"** block after Invariants & Rules — one line each for: PRD §6 (cgroup pin, un-deferred,
host change is a prerequisite); AGENTS.md logging rule (wrong on three counts, AD-9 supersedes,
refresh due after E1); AGENTS.md cgroup pitfall (superseded by AD-11, conditional on the host fix);
PRD FR-1/FR-2 error codes (changed by AD-3 despite `Delivered — no story`); proto `destination_ip`
(known exception to the Identity convention). A spine that silently contradicts its own inputs is
not a consistency contract — it is a second opinion.

---

## 6. Does it cover the driving PRD's capabilities?

**Yes, completely at the FR level.** All 32 FRs appear in the Capability → Architecture Map with a
home and a governing AD or convention; I checked each. `binds:` in the frontmatter lists all six
epics including the E5a/E5b split. PRD open questions Q3, Q4 and Q5 are closed with recorded reasons
and rejected alternatives; Q1 and Q2 are correctly carried to Deferred with revisit triggers. The
§7 exclusions are reproduced as exclusions rather than quietly converted into deferrals.

Two gaps in the *quality* of that coverage:

**Q1 — FR-27 (metrics) has a map row but no governance. (medium)**
It is filed under "Observability … governed by AD-9", and AD-9 is exclusively about logging. Nothing
fixes whether the metrics endpoint is the authenticated default or plain HTTP, which port, or
whether it is enabled at all — and that choice has an RBAC consequence that collides with AD-10
(F3). FR-27 is also first in the PRD's cut order, so the cheapest resolution is a Deferred-adjacent
line: "FR-27 serves the `controller-runtime` default on `:8080` without auth; if the secured
endpoint is used, AD-10's RBAC list gains TokenReview/SubjectAccessReview."

**Q2 — the cross-cutting resource ceiling is absent from the spine entirely. (medium)**
PRD §5 makes "the whole system … must run on a host with under 4 GB of usable RAM" a cross-cutting
NFR, and the addendum documents it costing a session (rust-analyzer at 1.3 GB of 3.7 GB). It shapes
at least three things the spine *does* decide: why NetworkPolicy enforcement is excluded (Calico
does not fit), why `replicas: 1`, and — unaddressed — whether FR-25's `testcontainers-go` run
(kind + Postgres + gateway + a second Postgres + a second gateway) fits alongside a running cluster
at all. E4's author will discover this empirically. One line in AD-11 naming the ceiling, and one in
the Verification map row saying integration tests assume the kind cluster is down, would close it.

---

## 7. Is every dimension this altitude owns decided, deferred, or open?

| Dimension | Status | Note |
| --- | --- | --- |
| Domain model / identity | Decided | Conventions: Identity, Naming — but see C5. |
| State ownership & authority | Decided | AD-1. Exemplary. |
| Persistence & data access | Decided | AD-2 (with the F1 hole). |
| Schema lifecycle | Decided, then reopened | Conventions: Schema vs Deferred D2. Duplicate source (M4). |
| API contract & codegen | Decided | AD-4. No staleness gate (D3). |
| Failure taxonomy | Decided | AD-3. Override unflagged (C3). |
| Dependency direction | Decided | AD-5, AD-7. |
| Cross-pass state | Decided | AD-6. |
| Service discovery | Decided | AD-10 — port unspecified (M1). |
| AuthZ / RBAC | Partly decided | Verbs listed; **scope** (Role vs ClusterRole) silent (M2); collides with FR-27 (F3). |
| Transport security | Excluded | PRD §7, correctly reproduced. |
| **Deployment & packaging** | **Decided** | AD-8. Image-tag rollout trap unstated (M5). |
| **Environments & infra strategy** | **Decided** | AD-11 — one kind cluster, registry-free, loopback-only. **Not silent.** Premise false (C1). |
| **Operations** | Partly decided | Health/probes and logging are decided. Operator replicas (M3), metrics (Q1), and the memory ceiling (Q2) are silent. |
| Observability — logging | Decided | AD-9. |
| Observability — metrics | **Silent** | Map row only (Q1). |
| Configuration | Decided | Conventions: Config. |
| Health semantics | Decided | Conventions: Health. Correctly ratifies shipped behaviour. |
| Secrets | Decided | Conventions: Secrets. |
| Idempotency / concurrency | Decided | Conventions: Idempotency; paradigm statement. |
| Verification strategy | Partly decided | Go side implied by AD-5/AD-6; Rust side and envtest/testcontainer provisioning silent (M6, Q2). |
| Repository structure | Seeded | Structural Seed. `docs/adr/` appears in the capability map but not in the seed — trivial, fix one or the other. |

**No whole dimension of the operational envelope is silent** — which is the failure this rubric item
hunts for, and the spine passes it. AD-11 is one of the better-earned ADs in the document precisely
because it commits to the unglamorous things (one cluster, one node, no registry, loopback,
`IfNotPresent`, both pods hardened) that stories otherwise assume differently. The envelope's
weakness is not absence; it is that its central factual premise is not true yet (C1) and that
operator-side operational settings were not carried over from the gateway's.

---

## Findings, ranked

| # | Severity | Finding | Fix size |
| --- | --- | --- | --- |
| C1 | **critical** | AD-11 states the cgroup-v1 host fix as done; host still mounts v1 and `.wslconfig` does not exist. Following AD-11 today breaks `make cluster-up`, and the prerequisite is owned by no epic. | 2 lines |
| C2/C3/C4/C5 | **high** | Four deliberate overrides (AGENTS.md logging, AGENTS.md cgroup pitfall, PRD §6, delivered FR-1/FR-2 error codes) plus the `destination_ip` exception live only in `.memlog.md`. Add an **Overrides** block. | ~10 lines |
| M1 | **high** | `<port>` in AD-10's DNS rule is undefined; `gatewayRef` carries name+namespace only. E2 and E3 will answer it separately. | 1 clause |
| M2 | **high** | Operator watch scope / RBAC scope (Role vs ClusterRole) unstated; interacts with AD-10's "any namespace". | 1 clause |
| M3/D1 | **high** | Operator replica count unpinned while leader election is deferred — deferral is only safe at one replica. | 5 words |
| F1 | **high** | AD-2's rule names `sqlx::query!` only; `health.rs:59` runs `sqlx::query("SELECT 1")` outside the store and complies. Say "no SQL in any form". | 1 clause |
| C5 | **high** | Identity convention claims `local_ip` "everywhere"; the shipped proto says `destination_ip`. `ListRoutes`' message shape is consequently undecided. | 2 lines |
| F3 | med–high | AD-10's closed RBAC list collides with FR-27's authenticated metrics endpoint. Narrow to the claim the rule supports. | 1 clause |
| D2 | medium | "Migration Job — revisit while building the chart" reopens a decision inside the epic (E5a) about to build it. | close it or move the trigger |
| D3 | medium | No local gate on AD-2/AD-4/AD-9's greppable rules; AD-4's staleness case is honour-system. Fold `make proto && git diff --exit-code` into `make check`. | 1 line |
| F2 | medium | AD-2's write-ordering half is review-enforced only, presented at the same strength as its greppable half. | say which, or make it structural |
| Q2 | medium | The <4 GB resource ceiling (PRD §5) is absent; it governs AD-11 and FR-25's testcontainers footprint. | 2 lines |
| Q1 | medium | FR-27 has a map row but no governing decision. | 1 line |
| M4 | medium | Schema DDL duplicated between `migrations/` and the initdb ConfigMap, with no single-source rule before E5a templates it. | 1 clause |
| M5 | medium | The `:dev` + `IfNotPresent` no-rollout trap survives the move to Helm, unstated. | 1 clause |
| C3 | medium | AD-3 changes semantics the PRD marks `Delivered — no story`; the epic generator is told not to touch that handler. | covered by Overrides block |
| M6 | low–med | Zero Rust tests while E1 rewrites two delivered handlers; recorded neither as decision nor accepted risk. | 1 line |
| F4 | low | AD-6's "over-reading that redials" is not a failure mode of FR-32. | reword |
| V1 | low | Stack groups `prost` at 0.14.6; `Cargo.lock` has 0.14.4. | fix row |
| V2 | low | `tracing-subscriber` listed in Stack but absent from `Cargo.toml`. | note as required addition |
| V3 | low | "pinned by digest" names no digest; `kind/cluster.yaml` still on `v1.33.1` untagged by digest. | supply it with C1 |
| — | low | `docs/adr/` in the capability map, absent from the Structural Seed. | fix either |
| — | low | AD-8 deletes `k8s/`, and AGENTS.md names those point-of-use comments as the design record (replicas:1 rationale, netpol header, probe split). Say the comments move with the manifests. | 1 clause |

## What not to change

- Do not lengthen the Design Paradigm section. The three-state table plus the three carried rules is
  the highest-value paragraph in the document.
- Do not expand AD-1, AD-5 or AD-7. They are correctly sized and mechanically checkable.
- Do not add sections. Every fix above is an edit inside an existing AD, Convention row, or Deferred
  bullet — except the Overrides block, which replaces content currently stranded in the memlog and
  is the one addition worth its lines.
- Do not soften the §7 exclusions into deferrals. The spine's discipline about "excluded, not
  deferred" is correct and rare.
