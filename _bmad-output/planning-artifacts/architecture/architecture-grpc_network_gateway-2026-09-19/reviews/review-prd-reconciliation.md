# Input-Reconciliation Review — PRD → ARCHITECTURE-SPINE

**Reviewer:** input-reconciliation lens
**Date:** 2026-09-20
**Input:** `prds/prd-grpc_network_gateway-2026-09-19/prd.md` + `addendum.md`
**Output under review:** `architecture-grpc_network_gateway-2026-09-19/ARCHITECTURE-SPINE.md`
**Also consulted:** the spine's `.memlog.md`, and the repository at `4f5fa3f` for facts the spine asserts

---

## Verdict

The spine is a strong translation of the PRD's *substance*. All 32 FRs are reachable through the
Capability → Architecture Map; the five SM-3 ideas each have a home; the PRD's sharpest quiet
requirements (`GetGatewayStatus` must never substitute for `ListRoutes`; an error must never render
as an empty result; liveness must never acquire a database dependency) all landed, two of them
nearly verbatim.

What did not land is a *class* of thing rather than a topic: **the spine drops the PRD's negative
space.** Every decision that costs the PRD something — an override, an accepted schedule slip, added
scope with no FR behind it — was reasoned through correctly in the memlog and then arrived in the
spine stripped of the fact that it was a cost. The memlog says "OVERRIDES PRD section 6"; the spine
says the host "runs the unified cgroup hierarchy" as ambient fact. The memlog says "Accepted risk:
Phase A stretches past PRD section 9's weekend budget"; the spine says nothing. Downstream epics
read the spine, not the memlog.

Two of the PRD's three counter-metrics (SM-C1 surface area, SM-C3 time-before-first-push) appear
nowhere in the spine, and the spine's net effect on both is additive.

---

## 1. FR Coverage — all 32

Every FR ID appears in the Capability → Architecture Map. Roll call by map row:

| Map row | FRs | Verdict |
| --- | --- | --- |
| Control-plane API | 1, 2, 3, 31 | Governed — AD-1/2/3/4, deeply |
| Packet path / pod-local view | 4, 5 | Governed — AD-1 |
| Runtime and health | 6, 7, 8 | Governed — Config + Health conventions |
| Image, cluster, hardening | 9, 10, 11 | **Partial** — see 1.2 |
| Packaging | 12, 23 | Governed — AD-8, AD-11 |
| `VpnTunnel` API | 13, 14 | Governed — AD-10 + Naming/Identity/Conditions |
| Reconciliation | 15–20, 32 | Governed — AD-1/5/6/7 |
| Gateway access and RBAC | 21, 22 | Governed — AD-3, AD-5, AD-10 |
| Verification | 24, 25 | Governed — AD-5, AD-6 |
| Observability | 26, 27 | **Partial** — see 1.1 |
| Legibility | 28, 29, 30 | Governed — AD-8, AD-11 |

**Nothing fell through entirely.** Two rows are nominal rather than substantive:

### 1.1 FR-27 (controller metrics) is mapped to an AD that does not govern it — LOW

Observability maps FR-26 and FR-27 jointly to AD-9, but AD-9 is titled and scoped to *structured
logging* only. FR-27's actual content — the `controller-runtime` metrics endpoint served and
reachable, the metrics port declared on the pod spec — has no rule anywhere in the spine, and the
chart (AD-8) never mentions a metrics port or Service. Low severity only because FR-27 is first on
PRD §9's cut list.

### 1.2 FR-9's hermeticity is only half-governed — HIGH

PRD §5 states "Builds are hermetic. Neither image requires a reachable database or a live cluster
to build. Base images pinned by digest." The spine governs the **Go** half of this precisely
(AD-4: "`go build` and `go test` must never require `protoc`") and drops the **Rust** half entirely.
Nowhere in the spine does `SQLX_OFFLINE`, the committed `.sqlx/` metadata, or Dockerfile base-image
digest pinning appear. AD-11 pins `kindest/node` by digest, which is a different artifact.

This is not cosmetic. Verified in the repo: `Dockerfile:26` sets `ENV SQLX_OFFLINE=true`, both bases
are `@sha256`-pinned, and `.sqlx/` holds three committed query files. E1 adds a new `ListRoutes`
query and extracts all SQL into `src/store/` (AD-2) — which invalidates and reshapes that metadata.
The failure mode is exactly the one AD-4 was written to prevent, one language over: change the SQL,
forget `cargo sqlx prepare`, and the image build breaks offline with an error that names the macro
rather than the cause.

**The spine is missing AD-4's Rust twin:** *any change to SQL regenerates `.sqlx/` in the same
commit; base images stay digest-pinned; the image builds with Postgres stopped.*

---

## 2. Open Questions (PRD §11) — the five

| Q | Status in spine | Cited by number? |
| --- | --- | --- |
| Q1 multi-replica writes | Deferred, revisit condition given ("before any change to `replicas`") | **Yes** — "PRD Q1" |
| Q2 cache-vs-durable observability | Deferred, revisit condition given ("the moment Q1 is answered") | **Yes** — "PRD Q2" |
| Q3 `ListRoutes` direct read vs re-hydrate | **Decided** — but never named | **No** |
| Q4 cross-namespace `gatewayRef` | **Decided** — "Any namespace is permitted" (AD-10) | **No** |
| Q5 Makefile vs Helm entry point | Closed by AD-8 | **Yes** — "PRD Q5" |

None is silently dropped. But the accounting is uneven in a way that matters:

### 2.1 Q3 — the one question explicitly routed to this pass is the one not acknowledged — MEDIUM

PRD §11 Q3 carries the tag *"Architecture decision — flag for `bmad-architecture`."* It is the
single item the PRD handed to this workflow by name. The spine **does** decide it correctly and
consistently: AD-1 ("every control-plane read comes from the store"), the state table ("Actual …
via `ListRoutes`"), the sequence diagram ("`select`, cache untouched"), and the Deferred entry
("Gateway-owned periodic re-hydration … deliberately not built and deliberately not folded into
`ListRoutes`"). The answer is there four times over.

What is missing is the label. Q1, Q2 and Q5 are all cited by number; Q3 is not. A reader walking
PRD §11 with the spine in hand can tick off four questions and will find nothing that says "this
closes Q3." The rejected alternative's reasoning — present and good in the memlog ("at replicas=1
it buys almost nothing … and it is the first thing a UJ-4 reviewer would challenge") — did not make
the transfer either, which also costs FR-29: that rationale is ADR material.

Also lost in transit: the PRD noted that re-hydration "would partly answer Q2." Choosing the direct
read is therefore *why* Q2 stays open. The spine defers Q2 and decides Q3 in two separate places
without connecting them.

### 2.2 Q4 — answered in the direction the PRD leaned against, without saying so — LOW

PRD Q4 frames cross-namespace `gatewayRef` sceptically ("RBAC implications … disproportionate to a
single-namespace MVP"). AD-10 answers "Any namespace is permitted" and dissolves the stated
objection properly — DNS construction means no Service read, so no extra RBAC — and the residual
risk is honestly booked under Deferred ("Cross-namespace consent … a real gap if that changes").
Substantively this is the better answer. It is just not flagged as having turned the PRD's lean
around, and Q4 is never named.

---

## 3. Assumptions (PRD §12) — the six

| Assumption | Spine | Verdict |
| --- | --- | --- |
| Q2 — cache/durable divergence acceptable at 1 replica, not surfaced | Deferred entry, consistent | Held; see 3.1 |
| FR-10 — aggregate Makefile target is FR-12's job, not a gap | AD-8 makes `make` the interface | Held |
| FR-13 — `localIP` immutable | Identity convention | Held |
| §5 — 3.7 GB WSL2 host remains the dev environment | **Silent** | See 4.1 |
| §8.1 — kubebuilder at `operator/`, independent Go module | Structural Seed + Stack | Held |
| §8.2 — `replicas=1` throughout MVP | AD-11 "gateway runs `replicas: 1`" | Held; minor tension below |

No assumption is contradicted outright. Two observations:

### 3.1 The spine narrows the PRD's Q2 reasoning without saying it is revising it — LOW

PRD Q2 characterises cache-vs-durable divergence as "a real gap the moment Q1 is answered," resting
on the glossary fact that hydration "runs until it first succeeds, then stops" — so nothing
re-syncs a pod's cache within its lifetime. The spine's Deferred entry substitutes a narrower
reading: "At one replica the window is narrow: an out-of-band delete through the gateway API clears
both, and the operator's re-create rewrites both."

That is true for UJ-2's demo path and false for the general case the PRD was describing (a direct
SQL mutation, or any divergence not routed through the gateway API, leaves the cache stale for the
pod's lifetime). The spine is not wrong about the MVP; it is quietly re-scoping the PRD's claim
from "real gap" to "narrow window" on the strength of one path. The load-bearing glossary fact —
hydration stops after first success — appears nowhere in the spine, so a reader cannot check the
reasoning.

### 3.2 `replicas: 1` as envelope vs. as chart value — LOW

AD-11 states `replicas: 1` as a fixed property of the operational envelope; FR-12 requires "Image
tag and Gateway replica count are **values**, not literals." Both are satisfiable (a value that
defaults to 1), but the spine states the constraint in the register that FR-12 forbids. One clause
fixes it.

---

## 4. Cross-cutting NFRs (§5) and Constraints (§6)

### NFRs — seven bullets

| NFR | Reflected |
| --- | --- |
| Idempotency is a system property | **Yes, strongly** — Idempotency convention, incl. "any RPC added later inherits this" |
| No external state cached across reconciles | **Yes, strongly** — AD-6, with a well-judged transport carve-out |
| The loop never blocks | Partly — AD-3/AD-5 imply it; the explicit "no `time.Sleep` in the reconcile path" prohibition is not stated |
| Least privilege throughout | **Yes** — AD-11 (both pods), AD-10 (RBAC minimal by construction) |
| Builds are hermetic | **Half** — Go side only; see 1.2 |
| **Resource ceiling (< 4 GB)** | **Absent — see 4.1** |
| Failure is legible | **Yes** — Conditions convention + AD-3 |

#### 4.1 The < 4 GB resource ceiling is gone, and the spine is the thing that added load — HIGH

PRD §5 makes the ceiling a first-class NFR with teeth: *"The whole system — kind node, Postgres,
Gateway, Operator, and the test harness — must run on a host with under 4 GB of usable RAM.
Anything that does not fit is deferred or dropped, and §7 says which."* It is the stated reason
enforced NetworkPolicy is a permanent non-goal (§7: Calico "does not fit the host's memory
ceiling"). It is a constraint that has already killed a feature.

The spine never mentions memory. Not in AD-11 ("the operational envelope is fixed"), which is the
one AD that exists to fix the envelope; not in the Stack table; not in Deferred.

Measured on this host today: **3810 MB total, 803 MB available.** Meanwhile the spine adds, against
that unchanged ceiling: a Go toolchain and build cache, a second container image, an operator pod
in-cluster, `envtest` (its own apiserver + etcd binaries), and `testcontainers-go` standing up a
Gateway *and* a Postgres container alongside a running kind cluster for FR-25. The addendum already
warns that `rust-analyzer` alone held 1.3 GB and must be stopped before creating a cluster.

AD-11 is the natural home: it already fixes cluster count, node count, replica count and port
binding. It should also state the ceiling and name what it forbids — most concretely, that FR-25's
`testcontainers-go` run and a live kind cluster are not assumed to coexist.

### Constraints — five bullets

| Constraint | Reflected |
| --- | --- |
| Dev credentials only | **Yes** — Secrets convention, with "labelled as such in the manifests" |
| Schema seeded, not migrated | **Yes** — Schema convention + Deferred carries the PM note's revisit-at-FR-12 |
| **Kubernetes version pinned / cgroup fix deferred** | **OVERRIDDEN — see 4.2** |
| No registry | **Yes** — AD-11 |
| Published ports bind loopback | **Yes** — AD-11 |

#### 4.2 The cgroup override reads as an oversight, and asserts a host state that is not true — CRITICAL

The PRD is unusually explicit here. §6: *"A permanent host-side fix exists (`cgroup_no_v1=all` in
`.wslconfig`) and is **deliberately deferred**."* The addendum §C repeats it: *"Permanent
alternative, deliberately deferred."* This is a decision the PRD took on purpose and recorded twice.

AD-11 reverses it in a subordinate clause:

> node image pinned **by digest** at v1.37.0 — the host runs the unified cgroup hierarchy
> (`kernelCommandLine = cgroup_no_v1=all` in `.wslconfig`), so the 1.33 pin is retired.

Three problems, in ascending order.

**(a) The override is not declared.** The clause is written in the present indicative, as a fact of
the environment that happens to make the old pin unnecessary. Nothing signals that a PRD constraint
was reversed. The spine's own memlog (line 25) states it plainly — *"OVERRIDES PRD section 6, which
deliberately deferred this fix, and AGENTS.md's pitfall telling the reader to leave the pin at
v1.33.1 — both need updating"* — and that sentence, which is the entire point, did not reach the
document. The reasoning behind the override is sound and better than the PRD's position (memlog
line 24: v1.33.1 is below kind v0.33.0's prebuilt floor, and four minors skewed from the installed
kubectl). It deserves to be stated as a reversal and won on its merits, not slipped in.

**(b) The host state asserted is false as of this review.** Verified: `stat -fc %T /sys/fs/cgroup`
returns `tmpfs` with a single `cgroup2` mount — the hybrid v1 layout, not the unified hierarchy.
`kind/cluster.yaml` still pins `kindest/node:v1.33.1` and still carries the comment *"Drop this pin
once the host runs the unified hierarchy."* The `.wslconfig` change has not been applied. AD-11
states as accomplished a prerequisite that is pending.

**(c) It is handled inconsistently with the spine's own best moment.** The closing line gets this
exactly right: *"Not deferred, and not an architecture decision: **Go is not installed on this
host.** Phase B cannot begin without it."* That is the correct register for a pending host
prerequisite — named, scoped, blocking. The cgroup change is the same class of item and got the
opposite treatment. Moving it to that closing section, alongside Go, would fix (a), (b) and (c)
together.

**(d) The override has an unpriced cost to SM-2.** See 6.2.

---

## 5. Non-Goals (§7) — does the spine permit anything excluded?

| Non-goal | Spine | Verdict |
| --- | --- | --- |
| Enforced NetworkPolicy | Deferred: "excluded, not deferred … manifests stay with their header" | Honoured, well |
| CKAD/CKA | Absent | Correctly not an architecture concern |
| A real data plane | Listed as excluded; `RoutePacket` stays a classifier on the derived cache | Honoured |
| Production readiness (TLS, managed secrets, HA Postgres, backup) | TLS listed as excluded; Secrets convention is dev-only; no HA anywhere | Honoured |
| PodDisruptionBudget | Listed as excluded | Honoured |
| Rust operator (`kube-rs`) | Not mentioned, but Go is mandated structurally throughout | Honoured by construction |
| Multi-tenancy / multi-cluster / cross-ns beyond `gatewayRef`'s explicit namespace | AD-10: "Any namespace is permitted" | Within bounds — see below |

**Nothing excluded is accidentally permitted.** The one item worth a second look is AD-10. §7
excludes "cross-namespace Gateway references **beyond `gatewayRef`'s explicit namespace**," which by
its own wording permits an explicitly named namespace — so AD-10 is inside the line. But AD-10's
phrasing, "Any namespace is permitted," is a broader-sounding grant than §7 invites, and a story
author reading only the spine could take it as licence toward multi-tenancy. The Deferred entry on
cross-namespace consent is what keeps this honest; a cross-reference from AD-10 to it would close
the gap.

---

## 6. Success metrics (§10), including the counter-metrics

### 6.1 SM-1 — the demo beat

Supported. 30s resync default (Config convention), requeue on every pass (sequence diagram), FR-17
/ FR-25 / FR-30 all mapped. Not cited by number anywhere, unlike SM-2 which is cited twice — a
cosmetic asymmetry, no substantive gap.

### 6.2 SM-2 — 60-second clean-machine quickstart — the structurally weakest metric

SM-2 is the metric the spine engages with most explicitly (bound in AD-8 and AD-11), and AD-8's
decision is the right one: `make` is the single documented path, Helm is the mechanism underneath,
`k8s/` is deleted so there are never two ways to deploy. That closes PRD Q5 *and* removes the
legibility cost against UJ-4. Genuinely good.

Three unpriced pressures remain:

**(a) The cgroup override adds a host-level prerequisite to the clean-machine path.** Under the PRD's
v1.33.1 pin, a clean WSL2 machine — cgroup v1, as WSL2 ships — ran the quickstart unchanged. Under
AD-11's v1.37.0 pin, that same machine must first edit a Windows-side `.wslconfig`, run
`wsl --shutdown`, and restart its Linux environment before `make cluster-up` can work at all. That
is not a command in a quickstart; it is a host reconfiguration and a restart, and SM-2's bar is
"no undocumented step." AD-11 binds SM-2 and does not mention this. (The override may still be
right — memlog line 25 argues it *helps* SM-2, since v1.33.1 is below kind v0.33.0's prebuilt
floor and a clean machine would be pulling a node image its kind was not built for. Both effects
are real. Only one is written down.)

**(b) No-registry plus two images versus a 60-second bar.** AD-11 fixes "both images are built
locally and side-loaded; nothing is pulled." On a clean machine that means a cold Rust release
build plus a Go build before anything converges. Whether SM-2's "under 60 seconds of commands"
means wall-clock or keystrokes is left unresolved by the PRD, and the spine — which is the document
that made the envelope registry-free and two-image — does not resolve it either. If it means
wall-clock, SM-2 is unreachable by construction and someone should say so now rather than at E6.

**(c) AD-8 makes a cuttable FR load-bearing.** See 6.4 — this is the sharpest SM-2 finding.

### 6.3 SM-3 — the five ideas defensible

**Well supported.** Each of the five has an architectural home, and in most cases a home that makes
the idea *visible* rather than merely present:

- **Level-triggered convergence** — AD-6 plus the Design Paradigm's three-rules paragraph plus the
  sequence diagram's `alt absent or differing / else already correct`.
- **Idempotency** — Idempotency convention, with the forward-binding clause.
- **Finalizers** — Finalizer convention, including the ordering that makes it correct ("added before
  the first call that creates external state").
- **`observedGeneration`** — Conditions convention ("advances only after a successful converge").
- **Periodic resync** — Config convention + the diagram's closing `requeue after 30s`.

Two credits worth recording, because both are quiet requirements from the addendum rather than
numbered FRs:

- Addendum §F.2 calls the **liveness/readiness split "the strongest single item in the project."**
  It is not one of the five and has no FR demanding architectural treatment. The Health convention
  carries it anyway, and closes with "This split is load-bearing, not stylistic." That is the
  addendum's argument surviving into the spine intact.
- FR-3's `[NOTE FOR PM]` — "`GetGatewayStatus` … must not be substituted for it" — lands in AD-1
  almost verbatim, and addendum §A.3's re-create-storm warning lands in AD-3 as "An error is never
  rendered as an empty result." Both are among the most load-bearing sentences in the PRD.

One quiet item did not survive: addendum §F.2's closing **"rhyme"** — that the gateway-side cache
re-sync rejected in §A.3 is the same drift-correction the operator performs one layer up, "the same
idea answering the same class of problem at two levels of the system," flagged as *worth stating
unprompted*. The spine preserves the decision (Deferred: re-hydration "deliberately not built and
deliberately not folded into `ListRoutes`") but not the observation. This is SM-3 material — it is
precisely the kind of thing that makes an unprompted explanation sound like understanding rather
than recitation — and it is now carried by no document between here and the ADRs.

### 6.4 SM-C1 (surface area) and SM-C3 (time before first push) — absent, and the spine runs against both — HIGH

Neither counter-metric appears anywhere in the spine. That would be tolerable if the spine were
neutral on them. It is not.

**Against SM-C1** ("Adding surface area … makes the artifact worse, not better. Depth on the five
ideas is the entire value"), the spine adds, none of it demanded by an FR:

- **AD-9 mandates a Rust `tracing` migration** — "no `println!` survives E1." FR-26 is an
  *Operator-only* requirement; the PRD asks nothing of the gateway's logging. AD-9 binds "all ·
  FR-26" and thereby creates gateway work with no FR behind it. (It is defensible on its merits —
  `tracing` is a declared-but-unused dependency, and half-structured logging is a real
  inconsistency — but it is new scope, in the epic PRD §9 budgets as *a weekend*.)
- **AD-4 requires a committed `operator/internal/gatewaypb/` and a `make proto` target** — a new
  build path and a new commit discipline. Well justified; still surface.
- **AD-11's host cgroup change and node-image upgrade** — new work outside the repo.
- **An `InvalidSpec` condition reason** beyond the PRD's three. FR-14 says "at least," so this is
  permitted; but FR-13 states invalid input is rejected at `kubectl apply` and "never reach[es] the
  Operator," so `InvalidSpec` is an operator-side path for a case the CRD markers are supposed to
  make impossible.

AD-8 is the one clear reduction (`k8s/` is deleted rather than maintained alongside the chart).

**Against SM-C3** ("A polished-but-unpushed repo scores zero against every metric here"):

- The spine's `.memlog.md` line 26 records, explicitly, *"Accepted risk: Phase A stretches past PRD
  section 9's weekend budget."* The spine itself never says this. A schedule risk was knowingly
  accepted and then not transmitted to the document the epics are generated from.
- **PRD §9's cut order is reflected nowhere in the spine** — and this is the finding with the
  sharpest consequence. The cut order is: drop FR-27 first, **then FR-12 (fall back to raw
  manifests)**, then FR-24. But AD-8 makes FR-12 *structural*: `make deploy` runs
  `helm upgrade --install`, the README quickstart depends on it, and `k8s/` "disappears when FR-12
  lands." The Structural Seed has `charts/netgw/` and no `k8s/`. If the second cut is ever
  exercised, AD-8 collapses, the fallback it was cut back to no longer exists in the tree, and SM-2
  loses its documented path. The PRD anticipated exactly this and left a `[NOTE FOR PM]` at §9:
  *"if the cut order is ever exercised, revisit SM-2 (which rests on FR-12) and SM-5 (which rests on
  FR-24)."* That note did not transfer. AD-8 needs one sentence saying what the Makefile interface
  degrades to if FR-12 is cut — the interface can survive the cut (`make deploy` calls `kubectl
  apply -k k8s/` instead), but only if someone writes that down before `k8s/` is deleted.

**SM-C2** (test coverage percentage) is honoured: the spine speaks only of named behaviours and a
counting fake, never of a coverage number.

### 6.5 SM-4, SM-5

SM-4 is not an architecture concern. SM-5 ("nothing orphans") is supported by the Finalizer
convention's ordering clause and the FR-18/FR-24 mappings — though it inherits the FR-24 half of
the untransferred cut-order note above.

---

## Findings, ranked

| # | Severity | Finding |
| --- | --- | --- |
| 1 | **CRITICAL** | AD-11 reverses PRD §6's *deliberately deferred* cgroup fix without declaring it an override, and asserts in the present tense a host state that is not yet true (host is still cgroup v1 hybrid; `kind/cluster.yaml` still pins v1.33.1). The memlog declares the override plainly; the spine does not. Inconsistent with the spine's own correct handling of the Go prerequisite. |
| 2 | **HIGH** | PRD §9's cut order is absent, and AD-8 makes FR-12 — the *second* item on that list — structural to the deploy interface and to SM-2, with `k8s/` scheduled for deletion. The PRD's "revisit SM-2 if the cut order is exercised" note did not transfer. |
| 3 | **HIGH** | The §5 resource ceiling (< 4 GB; 3810 MB total / 803 MB available measured today) appears nowhere, while the spine adds a Go toolchain, a second image, an operator pod, `envtest` and `testcontainers-go` against it. It is the NFR that already killed a feature (§7 enforced NetworkPolicy). AD-11 is its natural home. |
| 4 | **HIGH** | §5's hermetic-build NFR is governed for Go (AD-4, `protoc`) and dropped for Rust: no rule covers `SQLX_OFFLINE`, committed `.sqlx/`, or digest-pinned Dockerfile bases — despite E1 adding a query and moving all SQL. AD-4 needs its Rust twin. |
| 5 | **HIGH** | SM-C1 and SM-C3 appear nowhere, and the spine's net effect is additive against both — most concretely AD-9's gateway-wide `tracing` migration, which is new scope with no FR behind it, landing in the epic the PRD budgets as a weekend. The memlog's "accepted risk: Phase A stretches past PRD §9's budget" never reached the document. |
| 6 | **MEDIUM** | Q3 — the one open question the PRD explicitly routed to `bmad-architecture` — is decided correctly four times over but never cited by number, and its rejected-alternative rationale (present in the memlog, and ADR material for FR-29) did not transfer. Q1, Q2 and Q5 are all cited; Q3 and Q4 are not. |
| 7 | **LOW** | The spine narrows PRD Q2's "real gap" to "the window is narrow" on the strength of one path (UJ-2's API-routed delete), without flagging the revision, and omits the glossary fact it rests on (hydration stops after first success). |
| 8 | **LOW** | FR-27's actual content (metrics endpoint, declared metrics port) is mapped to AD-9, which governs logging only. Nothing in the spine or chart covers it. |
| 9 | **LOW** | Addendum §F.2's "rhyme" — gateway cache re-sync is the operator's drift-correction one layer down, flagged as *worth stating unprompted* — is not carried forward by any document. SM-3 material. |
| 10 | **LOW** | Minor: `docs/adr/` appears in the Capability map but not in the Structural Seed; AD-11 states `replicas: 1` as a fixed envelope where FR-12 requires it be a chart value; AD-10's "Any namespace is permitted" reads broader than §7 invites and should cross-reference the cross-namespace-consent deferral; FR-20's explicit "no `time.Sleep` in the reconcile path" prohibition is implied by AD-3 but never stated. |

---

## Recommended edits (minimal, all inside the existing structure)

1. **Move the cgroup fix out of AD-11's prose** and into the closing prerequisites section beside
   the Go note: *"Not an architecture decision, and a prerequisite: this host still mounts cgroup
   v1. Phase A's cluster work requires `kernelCommandLine = cgroup_no_v1=all` in `.wslconfig` and
   `wsl --shutdown`. This overrides PRD §6, which deferred the fix deliberately; the reason is that
   v1.33.1 is below kind v0.33.0's prebuilt floor and four minors skewed from the installed kubectl.
   `kind/cluster.yaml` and AGENTS.md both still carry the old pin and its pitfall."*
2. **Add one clause to AD-8:** what `make deploy` degrades to if FR-12 is cut (`kubectl apply -k
   k8s/`), and that `k8s/` is deleted only once the chart is green.
3. **Add the ceiling to AD-11:** the < 4 GB envelope, and the one concrete consequence — FR-25's
   `testcontainers-go` run and a live kind cluster are not assumed to coexist.
4. **Add AD-4's Rust twin** (or one clause inside AD-2): SQL changes regenerate `.sqlx/` in the same
   commit; bases stay digest-pinned; the image builds with Postgres stopped.
5. **Cite Q3 and Q4 by number** where they are decided (AD-1 and AD-10 respectively), and carry the
   rejected re-hydrate-then-read rationale into the spine or straight into the FR-29 ADR list.
6. **Add one line under Deferred or Design Paradigm** acknowledging SM-C1/SM-C3: what the spine
   deliberately does not add, and that AD-9's `tracing` sweep is accepted scope beyond FR-26 with
   its cost to PRD §9's Phase A budget stated.
