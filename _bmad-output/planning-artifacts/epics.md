---
stepsCompleted: [1, 2, 3, 4]
inputDocuments:
  - '_bmad-output/planning-artifacts/prds/prd-grpc_network_gateway-2026-09-19/prd.md'
  - '_bmad-output/planning-artifacts/architecture/architecture-grpc_network_gateway-2026-09-19/ARCHITECTURE-SPINE.md'
  - '_bmad-output/planning-artifacts/prds/prd-grpc_network_gateway-2026-09-19/addendum.md'
  - '_bmad-output/planning-artifacts/prds/prd-grpc_network_gateway-2026-09-19/reconcile-build-plan.md'
---

# grpc_network_gateway - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for grpc_network_gateway, decomposing the requirements from the PRD, UX Design if it exists, and Architecture requirements into implementable stories.

**Authority notes carried from the inputs, binding on everything below:**

- **PRD §9 is the authority on epic sequence**, not the PRD's own section numbering. Seven epics: E1, E2, E3, E4, E5, E6, E7.
- **Epic numbering differs from the PRD and the spine after E4. Renumbered 2026-10-04** so that sprint tracking, which takes integer epic keys only, can carry every story. PRD/spine **E5a** is **Epic 5** here, **E5b** is **Epic 6**, and **E6** is **Epic 7**. Their stories were renumbered with them (5a.N to 5.N, 5b.N to 6.N, 6.N to 7.N). When the PRD or the spine says "E6" it means Legibility, which this document calls Epic 7.
- **Every FR carries a binding `Status:` line.** `Delivered — no story` FRs must not produce stories; they are inventoried here because the Operator's design depends on their exact semantics.
- **Where the PRD and the architecture spine disagree on a structural decision, the spine governs.** Five such overrides are recorded in the spine's *Where This Contradicts Its Inputs* table and are reproduced under Additional Requirements below.
- **No UX design contract exists, and none is expected.** The system's only human surfaces are `kubectl`, the Makefile and the README. The UX Design Requirements section below is deliberately empty.

## Requirements Inventory

### Functional Requirements

Thirty-four FRs. **Ten are `Delivered — no story`** (FR-1, FR-2, FR-4 through FR-11) and generate nothing. **Twenty-four are `To build`** and are the scope of this breakdown, matching PRD §8.1.

> Note: `prds/index.md` still says "32 FRs; 22 to build" — stale as of 2026-09-20, when FR-33 and FR-34 were added by the architecture pass. The PRD body is correct at 34/24.

#### To build — these generate stories

FR-3: The Operator can read every Tunnel the Gateway owns from `vpn_routes` rather than from the serving pod's routing table, via a new `ListRoutes` RPC returning a dedicated four-field `Route` message carrying `owner`; an unreachable database returns `UNAVAILABLE`, distinguishable from "no Tunnels exist".
FR-12: The Gateway, Postgres, and the `VpnTunnel` CRD install into a cluster from one Helm chart, with image tag and replica count as values and the CRD in `crds/`; `helm uninstall` leaves no namespaced objects behind except the PVC.
FR-13: A user can declare a Tunnel as a Kubernetes resource in group `net.lilianmrt.dev`, version `v1alpha1`, with `spec` carrying `gatewayRef`, `localIP`, `tunnelID`, `remoteEndpoint`; `localIP` is API-server-validated as IPv4 and immutable after creation.
FR-14: A user can see whether a Tunnel has converged from `kubectl get` alone — `status` as a subresource, printer columns for local IP, tunnel ID, Ready and age, and `status.conditions` in standard `metav1.Condition` shape.
FR-15: The Operator watches `VpnTunnel` resources and reconciles one resource per pass; a Reconcile for a resource that no longer exists returns cleanly without a Gateway call.
FR-16: Reconcile computes what must change before changing anything and converges from any starting state — absent, present-but-wrong, or already correct — with an already-matching Tunnel producing **no** gRPC write call, verified by call count.
FR-17: Reconcile runs periodically even when nothing in the cluster changed, every Reconcile returning a requeue-after interval defaulting to **30 seconds** and configurable through the Helm chart; a Tunnel deleted out of band is restored within that interval.
FR-18: Deleting a `VpnTunnel` removes its Tunnel from the Gateway before the resource is collected, via a finalizer added **before** the first call that creates external state.
FR-19: `status.observedGeneration` is set to `metadata.generation` only after a successful converge, and the `Ready` condition's `reason` distinguishes converged from every failure mode the Operator can observe.
FR-20: Errors are handled by returning them to `controller-runtime` for exponential backoff; no `time.Sleep` appears in the reconcile path.
FR-21: The Operator connects to the Gateway named by each resource's `gatewayRef`, resolved to in-cluster Service DNS, with a timeout on every call and connection failure surfacing as a typed error.
FR-22: The Operator runs with least-privilege RBAC generated from kubebuilder markers, on a dedicated ServiceAccount, covering `vpntunnels`, `vpntunnels/status` and event emission and nothing else.
FR-23: The Operator's Deployment, ServiceAccount and RBAC are added to the chart FR-12 established, built and side-loaded by the same Makefile flow, carrying the FR-11 hardening posture, with the resync interval as a chart value.
FR-24: Controller behaviour is verified with `envtest` — converge-from-absent, converge-from-wrong, no-op on already-correct, finalizer removal on delete, `observedGeneration` lagging a spec edit, and an idempotency assertion on write-call count — running without a cluster on a clean machine.
FR-25: Integration tests run the Gateway and Postgres as containers via `testcontainers-go`, covering a full create → `ListRoutes` → delete → re-delete cycle, the UJ-2 drift case, and the FR-31 regression.
FR-26: The Operator logs through `logr` with the resource's namespace and name on every reconcile line, and the gRPC status code on Gateway call failures.
FR-27: The standard `controller-runtime` metrics endpoint is served and reachable — reconcile count, error count and queue depth scrapeable, with the metrics port declared on the pod spec.
FR-28: A README a reader can understand from the first screen and run from the quickstart — architecture diagram, a quickstart reaching a converged Tunnel in **under 60 seconds of commands** verified on a clean environment, and each of the five control-plane ideas stated in a standalone paragraph.
FR-29: Two or three ADRs under a page each, stating the rejected alternative: why an operator rather than a CLI, why Go for the Operator and Rust for the Gateway, why periodic resync is required rather than optional.
FR-30: A recording showing UJ-2 end to end — `kubectl apply`, the resource going Ready, an out-of-band delete, and automatic restoration — embedded in the README, not only linked.
FR-31: `CreateVpnTunnel` writes the durable record before the in-memory routing table, matching the ordering `DeleteVpnTunnel` already uses; a failed database write leaves the routing table unchanged.
FR-32: Reconcile reads actual state from `ListRoutes` at the start of every pass and retains nothing about the Gateway between passes — no Gateway-derived state in any field, package variable or cache outliving one call.
FR-33: The same underlying failure returns the same gRPC status code from every RPC — `UNAVAILABLE` for an unreachable database from all three write and read paths, `INVALID_ARGUMENT` for any Postgres constraint violation, `INTERNAL` reserved for genuinely unexpected failures — classified in exactly one package per side and never by string-matching a message.
FR-34: A Tunnel row records which `VpnTunnel` owns it (`<namespace>/<name>` in an `owner` column, carried on create and delete requests); creating over a differently-owned local IP returns `FAILED_PRECONDITION` and changes nothing, deletes remove only owned rows, and the conflict surfaces as a permanent `Ready=False` reason.

#### Delivered — no story, inventoried for the semantics downstream work depends on

FR-1: Create a Tunnel by local IP, tunnel ID and remote endpoint. Upserts on conflict, so re-applying an unchanged spec is a no-op — this is what lets FR-16 handle create and change with one RPC. *Known defect: in-memory write precedes the database write; corrected by FR-31. Error codes amended by FR-33; ownership added by FR-34.*
FR-2: Delete a Tunnel by local IP, idempotently — an absent Tunnel returns `success=true, existed=false`, never an error. Postgres delete precedes routing-table removal. A finalizer and a retrying Reconcile both depend on this. *Error codes amended by FR-33; ownership scoping added by FR-34.*
FR-4: `GetGatewayStatus` reports the serving pod's in-memory routing table, explicitly not a cluster-wide view. Debugging and demonstration surface only — **must never be substituted for `ListRoutes`**.
FR-5: `RoutePacket` classifies an IPv4 packet against the routing table — `FORWARDED`, `DROPPED (NO_ROUTE)`, or `DROPPED (MALFORMED_HEADER)`, all returning OK with an action string. The data plane is simulated.
FR-6: The Gateway reads `DATABASE_URL` (mandatory, no compiled-in default) and `BIND_ADDR` (defaulting to `0.0.0.0:50051`) from the environment; a local `.env` is optional.
FR-7: The Gateway starts, binds and serves health regardless of whether Postgres is reachable — lazy pool connection, hydration retried by a background task until it first succeeds.
FR-8: Liveness and readiness are distinct gRPC health entries. Readiness (`""`) follows the database and pulls the pod from the Service in roughly 15 seconds; liveness (`liveness`) is set once at startup and never depends on the database.
FR-9: A hermetic, minimal container image — builds with Postgres stopped via `SQLX_OFFLINE=true`, both base images pinned by digest, distroless, `nonroot` uid 65532, under 30 MB (27.9 MB verified).
FR-10: A kind cluster runs the Gateway, Postgres with a PVC, and their configuration, reachable from host tooling on a loopback-bound NodePort with no `port-forward`, driven by named Makefile targets.
FR-11: The Gateway pod runs with least privilege — `runAsNonRoot` with explicit uid, all capabilities dropped, no privilege escalation, `RuntimeDefault` seccomp, read-only root filesystem, and NetworkPolicies headed with a statement that kind's CNI does not enforce them.

### NonFunctional Requirements

NFR1: **Idempotency is a system property, not a feature.** Every Gateway write and every Reconcile must be safe to repeat. Any RPC added later inherits this. *(PRD §5; spine conventions: Idempotency)*
NFR2: **No external state is cached across reconciles.** Nothing Gateway-derived survives a pass — this is what makes the loop level-triggered rather than edge-triggered. A `grpc.ClientConn` is transport, not state, and is exempt. *(PRD §5, FR-32; AD-6)*
NFR3: **The loop never blocks.** No sleeping inside Reconcile; all waiting is expressed as requeue or as call timeouts. *(PRD §5, FR-20, FR-21)*
NFR4: **Least privilege throughout**, applying to both the Gateway and the Operator pods — non-root with explicit uid, all capabilities dropped, read-only root filesystem, `RuntimeDefault` seccomp, generated RBAC. *(PRD §5; AD-11)*
NFR5: **Builds are hermetic.** Neither image requires a reachable database or a live cluster to build; base images pinned by digest, never by tag. *(PRD §5; spine conventions: Hermetic builds)*
NFR6: **Resource ceiling under 4 GB.** The whole system — kind node, Postgres, Gateway, Operator, and the test harness — must run on a host with under 4 GB usable RAM. The Go toolchain, a second image, the operator pod, `envtest` and `testcontainers-go` all land against that ceiling. *(PRD §5; AD-11. Note: the reconcile review flags the PRD's "anything that does not fit is a non-goal, not a deferral" as overstated — the host fix is an escape hatch, and AD-11 now makes it a prerequisite.)*
NFR7: **Failure is legible.** Every failure mode a user can hit surfaces in `kubectl get`/`describe` output with a distinguishing reason, not only in Operator logs. *(PRD §5, FR-19)*
NFR8: **Liveness must never acquire a database dependency**, in this or any future change. Restarting a process because its dependency is down fixes nothing and converts an outage into a simultaneous restart storm across every replica. *(PRD §4.2 feature-specific NFR; spine conventions: Health — "this split is load-bearing, not stylistic")*

### Additional Requirements

Technical requirements drawn from the architecture spine that shape stories but are described by no FR.

**Starter template.** No greenfield starter for the repository as a whole — the Rust Gateway exists and is partly delivered. **But the Operator is scaffolded with `kubebuilder` v4.16.0 at `operator/`, as a Go module independent of the Rust crate** (PRD §8.1 assumption; notes milestone M6, "enabling work, no FR of its own"). This scaffold is a prerequisite of E2's first story and has no FR to hang on.

**Prerequisites that the spine leaves unowned, each blocking its phase** (spine, closing line). **Decision 2026-09-20: both get stories, and both are marked `Performed by Lilian` — they are host-level changes an agent must not make.** A story of this kind states what must be true, how to verify it, and stops; it contains no agent-executed steps.
- **Go 1.27.1 is not installed on this host.** `>= 1.26` is required by controller-runtime. Blocks all of Phase B. Story sits at the head of E2, the first Go epic.
- **The AD-11 host cgroup fix has not been applied.** Verified 2026-09-20: no `cgroup_no_v1` in `/proc/cmdline`, 16 v1 controllers mounted, `kind/cluster.yaml` still pinning `v1.33.1` by tag. Requires `kernelCommandLine = cgroup_no_v1=all` in `.wslconfig` and `wsl --shutdown` — a Windows-side file edit and a WSL restart, outside the reach of anything running inside WSL. Owned by E5 per AD-11; unblocks the pin moving to `kindest/node:v1.37.0` by digest.

**Structural work E1 carries beyond its four FRs** (spine, PRD §9 "E1 grew in the architecture pass"):
- Extract every SQL statement into `src/store/`, including the readiness probe's `SELECT 1` (`src/services/health.rs:59`) as `store::ping()`, so persist-before-cache ordering is enforceable rather than remembered (AD-2).
- Replace every `println!` with `tracing`, adding `tracing-subscriber` as a dependency — without it every `tracing` macro emits nothing (AD-9).
- Refresh `AGENTS.md`, three of whose rules are false today (spine: "must be refreshed when E1 lands").

**Architecture invariants binding on stories:**
- **AD-1** — `vpn_routes` is the sole authority; the routing table is a per-pod derived cache. Every control-plane read comes from the store; every write persists before it caches.
- **AD-2** — No SQL reaches Postgres from outside `src/store/`. A handler may call the store then the cache, never the reverse. `network/router.rs` has no database knowledge and gains none.
- **AD-3** — One failure taxonomy mapped at the store boundary into retryable / permanent / conflict; Go classifies only in `operator/internal/gateway`. An error is never rendered as an empty result.
- **AD-4** — `proto/gateway.proto` and `.sqlx/` are generated artefacts that move in the same commit as their source. Go stubs are **committed** to `operator/internal/gatewaypb/`; `go build` and `go test` must never require `protoc`. `make check` runs the generators and fails on `git diff --exit-code`. **Applied 2026-10-04:** `make check` regenerates only the artefacts that need no database (Go stubs, CRD, RBAC). `.sqlx/` is checked by a separate `make check-sqlx`, which needs Postgres (Story 1.2).
- **AD-5** — `operator/internal/controller` must not import `google.golang.org/grpc`. Only `internal/gateway` dials, applies timeouts, or inspects a status code. Tests inject a counting fake.
- **AD-6** — Connections use `grpc.NewClient`, never `grpc.Dial` with `WithBlock` (which would make operator startup depend on gateway availability); cached per `gatewayRef` for the process lifetime and reused.
- **AD-7** — One-way dependency. The operator's only channel to tunnel state is the gRPC API; it never touches Postgres and never imports the generated stubs directly. The gateway never reads the Kubernetes API.
- **AD-8** — The Makefile is the interface, Helm is the mechanism, CRDs are applied outside Helm. `make deploy` runs `kubectl apply -f charts/netgw/crds/` **first**, then `helm upgrade --install`. `k8s/` is deleted when FR-12 lands. Chart is `apiVersion: v2`.
- **AD-9** — Both programs log structurally: Rust `tracing` + `tracing-subscriber`, Go `logr` through controller-runtime. Gateway lines carry the `local_ip` and, on failure, the gRPC code.
- **AD-10** — `gatewayRef` resolves by DNS to `<name>.<namespace>.svc.cluster.local:50051`, never by an API read. **The port is a package constant in `internal/gateway`, not a `gatewayRef` field.** Any namespace is permitted.
- **AD-11** — Operational envelope: one kind cluster `netgw`, single node; both images built locally and side-loaded, `imagePullPolicy: IfNotPresent`, nothing pulled from a registry; every published port binds `127.0.0.1`; Gateway and Operator both `replicas: 1`; the operator watches **cluster-wide**, so a `ClusterRole`. After the host fix, `kindest/node:v1.37.0` by digest; before it, `v1.34.11` is the only kind v0.33.0 prebuilt that boots.
- **AD-12** — `ListRoutes` returns `repeated Route`, a **new** four-field message keyed `local_ip`, carrying `owner`. The shipped `RouteDetails` is left alone. Bookkeeping columns — `id`, `created_at` — are not on the wire. "Already correct" is `owner` equal to the caller's and exact string equality of `tunnel_id` and `remote_endpoint` *(amended 2026-10-07)*.
- **AD-13** — Every row has one owner (`<namespace>/<name>`); deletes are owner-scoped; a conflicting owner is a permanent `Ready=False` reason, not a retry; FR-2's idempotency is unchanged.
- **AD-14** — **An orphan row is a legitimate steady state, not drift.** No story may add a reaper that sweeps `ListRoutes` and deletes unmatched rows — it would delete the very out-of-band route UJ-2's demo depends on. SM-5 is satisfied by finalizers, not by sweeping.
- **AD-15** — One validation authority: the CRD's markers, mirrored by the gateway at the same bounds. Any value reaching the database and violating a constraint is a **permanent** error, never retryable.

**Consistency conventions that must be applied identically across stories:**
- Naming: CRD `spec.localIP` / `spec.tunnelID` / `spec.remoteEndpoint` (Go initialisms uppercase in JSON tags); proto and SQL `local_ip` / `tunnel_id` / `remote_endpoint`. The mapping exists in exactly one place, `internal/gateway`.
- Go packages: `internal/controller`, `internal/gateway`, `internal/gatewaypb`; API types in `api/v1alpha1`.
- Finalizer: `net.lilianmrt.dev/tunnel-cleanup`.
- Conditions: type `Ready`; reasons at least `Converged`, `GatewayUnreachable`, `InvalidSpec`, `OwnedByAnother`, `Deleting`.
- Config: Operator reads flags in the controller-runtime idiom; both programs' config surfaces as Helm values; resync defaults to 30s.
- Schema: seeded through the Postgres image's init hooks. **AD-13's `owner` column is added to the seed, not as a migration.**
- Secrets: dev-only values, committed deliberately, labelled as such.

**Overrides the spine applies to the PRD** — where a story must follow the spine, not the PRD text:
| PRD says | Spine governs |
|---|---|
| §6: the cgroup host fix is "deliberately deferred" | AD-11: it is a prerequisite task owned by E5 |
| §6 and `AGENTS.md`: Kubernetes **1.36+** refuses cgroup v1 | Factually wrong — the cutover is **1.35** (KEP-5573); v1.35.8 would also fail |
| `AGENTS.md`: "use `log`, never `tracing`", "no new `println!`" | AD-9 — all three clauses are false today; `tracing` is the dependency. **Already corrected in `AGENTS.md` by 2026-10-04**: it now records `println!` as today's practice and AD-9 as the plan. Story 1.7 updates it again once E1 lands. |
| `AGENTS.md`: leave the node image pinned at `v1.33.1` | AD-11 — retired once the host fix lands. `AGENTS.md` already states the 1.35 cutover correctly. |
| FR-1, FR-2 marked `Delivered — no story` | AD-3 — E1 reopens both handlers to unify the taxonomy |

**Concrete details the reconcile review flags as missing from the PRD, to be carried into acceptance criteria:**
- ~~`status.appliedRoutes`~~ — **CLOSED 2026-09-20: out.** Named in the source notes' reconcile step 7 but never carried into FR-19. The `VpnTunnel` status carries `observedGeneration` and `conditions` only. No story adds this field, and SM-C1 (which penalises added surface area) is the reason.
- The CV bullet is already drafted verbatim and functions as a scope test — every clause maps to an FR: *"Converged declarative network state onto a live gateway by building a Kubernetes operator (Go, controller-runtime) with finalizers and drift-correcting reconciliation over a gRPC control-plane API."*
- **The demo beat is five seconds. CLOSED 2026-09-20.** UJ-2's "five-second proof" and the source notes agree; FR-30 and SM-1's "under ten seconds" is the drift. Acceptance criteria use **five**.
- `examples/smoke_client.rs` already walks create → observe → delete → confirm gone → delete again; it is the skeleton of both the FR-30 demo and the FR-25 integration test. `examples/health_probe.rs` mirrors a Kubernetes `grpc:` probe.

**Addendum §F, the brief for E7** — arguments the README and ADRs must make, not requirements: the liveness/readiness split is the strongest single item and was proved by observation; level-triggered convergence is the most important idea in the controller pattern and the one most often gotten wrong; finalizers are the concrete answer to "why can't you just watch for delete events"; periodic resync signals operational understanding because event-driven alone is the intuitive-but-wrong answer; `observedGeneration` is the smallest of the five but separates current status from stale. Plus the failure each delivered behaviour avoids (§F.1): `tonic_health` defaults `""` to SERVING unconditionally, and `PgPoolOptions::connect()` fails fast into CrashLoopBackOff where a readiness probe cannot help.

### UX Design Requirements

None. No UX design contract exists for this project and none is expected — the system's human surfaces are `kubectl` output (governed by FR-14's printer columns and conditions), the Makefile (AD-8), and the README (FR-28). Those are specified as functional requirements above.

### FR Coverage Map

All 24 `To build` FRs are mapped. The 10 `Delivered — no story` FRs are listed at the end for completeness and generate nothing.

| FR | Epic | What the epic does with it |
|---|---|---|
| FR-3 | **E1** | `ListRoutes` over `vpn_routes`, returning the dedicated `Route` message (AD-12) |
| FR-31 | **E1** | Persist before cache on create, matching delete's existing ordering |
| FR-33 | **E1** | One failure taxonomy across every RPC, mapped at the store boundary (AD-3) |
| FR-34 | **E1** | The `owner` column; owner-scoped creates and deletes (AD-13) |
| FR-13 | **E2** | The `VpnTunnel` CRD — group, version, spec fields, validation markers |
| FR-14 | **E2** | Status subresource, printer columns, `metav1.Condition` shape |
| FR-21 | **E3** | `internal/gateway` — DNS resolution, per-call timeouts, typed errors (AD-5, AD-6, AD-10) |
| FR-15 | **E3** | The reconcile loop over `VpnTunnel`, one resource per pass |
| FR-32 | **E3** | Fresh `ListRoutes` read every pass; nothing retained between passes (AD-6) |
| FR-16 | **E3** | Diff then apply; already-correct produces no write call |
| FR-19 | **E3** | `observedGeneration` and `Ready` reasons |
| FR-20 | **E3** | Return errors to controller-runtime; no sleeping in the loop |
| FR-26 | **E3** | `logr` with namespace and name on every line, gRPC code on failures (AD-9) |
| FR-17 | **E4** | Periodic resync at 30s, configurable — the drift correction of UJ-2 |
| FR-18 | **E4** | The `net.lilianmrt.dev/tunnel-cleanup` finalizer, added before external state |
| FR-24 | **E4** | `envtest` coverage of the six named controller behaviours |
| FR-25 | **E4** | `testcontainers-go` integration against a real Gateway and Postgres |
| FR-12 | **E5** | The Helm chart for Gateway, Postgres and the CRD; `k8s/` deleted (AD-8) |
| FR-22 | **E6** | Generated least-privilege RBAC on a dedicated ServiceAccount, cluster-wide (AD-11) |
| FR-23 | **E6** | The Operator's Deployment added to the chart, with FR-11 hardening |
| FR-27 | **E6** | The controller-runtime metrics endpoint, port declared on the pod spec |
| FR-28 | **E7** | README: diagram, 60-second quickstart, the five ideas as standalone paragraphs |
| FR-29 | **E7** | Two or three ADRs, each under a page, each naming its rejected alternative |
| FR-30 | **E7** | The UJ-2 recording, restoration visible in five seconds, embedded in the README |

**Not FR-derived, but scoped into epics because the work is real and otherwise unowned:**

| Work item | Epic | Source |
|---|---|---|
| Extract all SQL into `src/store/`, including `store::ping()` | **E1** | AD-2 |
| Replace every `println!` with `tracing`; add `tracing-subscriber` | **E1** | AD-9, Stack table |
| Update `AGENTS.md` once E1 lands — `tracing` only, no SQL outside `src/store/` | **E1** | Spine, *Where This Contradicts Its Inputs* |
| Install Go 1.27.1 — **performed by Lilian** | **E2** | Spine closing line |
| `kubebuilder` scaffold at `operator/` | **E2** | PRD §8.1, notes M6 |
| WSL cgroup v1 host fix — **performed by Lilian** | **E5** | AD-11 |
| Move the node image pin to `v1.37.0` by digest | **E5** | AD-11 |

**`Delivered — no story`:** FR-1, FR-2, FR-4, FR-5, FR-6, FR-7, FR-8, FR-9, FR-10, FR-11. Note that E1 nonetheless reopens the FR-1 and FR-2 handlers, per AD-3 and AD-13 — the FRs stay Delivered; FR-31, FR-33 and FR-34 carry the amendments.

## Epic List

Seven epics, following **PRD §9, which is the authority on sequence**. Each depends only on those before it, and none requires a later epic to function.

**Dependency correction 2026-10-04 (final validation).** Two of §9's "depends on nothing" entries do not survive the stories. E2 depends on E1, because Story 2.3's CRD markers mirror the validation bounds Stories 1.5 and 1.8 record (AD-15). E5 depends on E1 and E2, because Story 5.2 packages the CRD from Story 2.3 and seeds the `owner` column from Story 1.4. The order is unchanged. What is lost is §9's plan to build E5 alongside E1 in Phase A: E5 now follows E2.

### Epic 1: Durable actual-state reads
*Rust · Phase A · depends on nothing*

The Gateway becomes a control-plane API an external reconciler can trust: a caller can read the authoritative set of Tunnels independently of which pod answers, every failure means the same thing whichever RPC produced it, and no resource can destroy a Tunnel it does not own. Delivers standalone value before any operator exists — the known persist-after-cache defect is gone and the API stops lying about why it failed.

**FRs covered:** FR-3, FR-31, FR-33, FR-34
**Also carries:** SQL extraction into `src/store/` (AD-2), `tracing` replacing `println!` (AD-9), the `AGENTS.md` refresh
**Note:** §9 warns this epic has grown past its weekend budget. It is the one place I would look first if the cut order is ever exercised.

### Epic 2: The `VpnTunnel` API
*Go · Phase B · depends on E1 (the validation bounds of Stories 1.5 and 1.8)*

A user can declare a Tunnel as a Kubernetes resource and have the API server reject it if it is wrong, before any controller sees it. `kubectl get vpntunnels` renders a useful table. Nothing reconciles yet — and that is a coherent stopping point, because the API is the contract everything downstream is written against.

**FRs covered:** FR-13, FR-14
**Also carries:** the Go toolchain install (**performed by Lilian**), the `kubebuilder` scaffold at `operator/`

### Epic 3: Converge a tunnel
*Go · Phase B · depends on E1, E2 · realizes UJ-1*

`kubectl apply` makes a tunnel real. The Operator reads actual state fresh, diffs it against the spec, creates what is missing, writes honest status, and says why when it cannot — without ever blocking a worker or caching what it saw. This is the first epic a user can demonstrate end to end.

**FRs covered:** FR-21, FR-15, FR-32, FR-16, FR-19, FR-20, FR-26

### Epic 4: Drift and deletion
*Go · Phase B · depends on E3 · realizes UJ-2, UJ-3*

The loop becomes a control plane rather than an apply-once tool: a route deleted behind its back comes back within 30 seconds with no event involved, and deleting the resource actually deletes the tunnel instead of orphaning it. Verified against a real API server and a real Gateway.

**FRs covered:** FR-17, FR-18, FR-24, FR-25
**Guardrail:** AD-14 — no reaper. SM-5 is satisfied by finalizers removing what they created, never by sweeping `ListRoutes` for unmatched rows.

### Epic 5: Package the Gateway *(PRD E5a)*
*depends on E1 (Story 1.4's schema), E2 (Story 2.3's CRD)*

One command installs the Gateway, Postgres and the CRD into a clean cluster, and one removes them. The Makefile stays the human interface with Helm underneath it, and `k8s/` disappears.

**FRs covered:** FR-12
**Also carries:** the WSL cgroup fix (**performed by Lilian**), then moving the node image pin to `v1.37.0` by digest
**Note:** PRD §9 placed this in Phase A alongside E1. It can't stay there, because Story 5.2 ships the CRD and the `owner` schema, so it follows E2. Story 5.1 (the host fix) has no dependencies and can happen at any time. AD-8 has made this epic structural — cutting it now means reverting AD-8, not skipping an epic.

### Epic 6: Deploy the Operator *(PRD E5b)*
*Phase B · depends on E4, E5*

The Operator runs in the cluster with exactly the permissions its markers declare and reports on itself while it does. After this epic the system runs unattended from a single install.

**FRs covered:** FR-22, FR-23, FR-27

### Epic 7: Legibility *(PRD E6)*
*Phase C · depends on E6 · realizes UJ-4*

A reviewing engineer with four minutes and no context can tell what was built and why. README with the architecture on the first screen and a 60-second quickstart, ADRs for the three questions a reviewer would ask, and a recording where the restoration beat lands in five seconds.

**FRs covered:** FR-28, FR-29, FR-30
**Note:** §9 — never cut. An unbuilt FR costs less than an unreadable repo.

---

## Epic 1: Durable actual-state reads

The Gateway becomes a control-plane API an external reconciler can trust: a caller can read the authoritative set of Tunnels independently of which pod answers, every failure means the same thing whichever RPC produced it, and no resource can destroy a Tunnel it does not own. Delivers standalone value before any operator exists.

*Rust · Phase A · depends on nothing · FR-3, FR-31, FR-33, FR-34 · AD-1, AD-2, AD-3, AD-4, AD-9, AD-12, AD-13, AD-15*

### Story 1.1: Emit every gateway log line through `tracing`

As the gateway's maintainer,
I want every log line emitted through `tracing` with structured fields,
So that half the system stops printing to stdout while the other half claims to be observable, and every line added by later stories is structured from the start.

**Acceptance Criteria:**

**Given** `tracing-subscriber` is declared nowhere in `Cargo.toml` and every `tracing` macro therefore emits nothing,
**When** the dependency is added and a subscriber is initialised at startup,
**Then** `tracing` output appears on stdout,
**And** the existing `tracing` dependency stops being decorative.

**Given** the gateway source tree,
**When** `grep -rn 'println!' src/` is run,
**Then** it returns no matches.

**Given** a create, delete, or hydration event,
**When** a line is emitted for it,
**Then** the `local_ip` it concerns is a structured field, not interpolated into the message text.

**Given** a handler that is returning an error,
**When** the failure is logged,
**Then** the line carries the gRPC status code as a field.

**Given** the pod-restart evidence recorded in addendum §B,
**When** the process hydrates at startup,
**Then** a line equivalent to `hydrated N route(s) from the database` is still emitted, because FR-10's demonstration depends on it.

### Story 1.2: Move every SQL statement behind `src/store/`

As the gateway's maintainer,
I want all SQL to live in one module,
So that persist-before-cache ordering is enforced by a module boundary instead of remembered by whoever adds the next RPC.

**Acceptance Criteria:**

**Given** the source tree outside `src/store/`,
**When** it is searched for `sqlx::query!`, the function form `sqlx::query`, or any other path that reaches Postgres,
**Then** there are no matches.

**Given** the readiness probe's `SELECT 1` at `src/services/health.rs:59`,
**When** the probe runs,
**Then** it calls `store::ping()`,
**And** `health.rs` contains no SQL.

**Given** `src/network/router.rs`,
**When** it is inspected,
**Then** it holds no database knowledge and imports no sqlx,
**And** no store function calls into the cache — the dependency runs one way only.

**Given** a handler that must touch both,
**When** it executes,
**Then** it calls the store first and the cache second.

**Given** `.sqlx/` metadata that is now stale because every query moved,
**When** `make prepare` is run and the image is then built with `SQLX_OFFLINE=true` and Postgres stopped,
**Then** the build succeeds.

**Given** the generated-artefact rule (AD-4) and the fact that regenerating `.sqlx/` needs a live database,
**When** `make check-sqlx` is run with Postgres up,
**Then** it regenerates `.sqlx/` and fails on `git diff --exit-code`,
**And** `make check` stays runnable with Postgres stopped, as `AGENTS.md` promises, and does not touch `.sqlx/`,
**And** neither check depends on CI, which is deferred.

### Story 1.3: Persist before caching on create

As the gateway's administrator,
I want a failed database write to leave no route in memory,
So that a pod cannot serve a route that was never persisted.

*Realizes FR-31.*

**Acceptance Criteria:**

**Given** a valid create request,
**When** `CreateVpnTunnel` executes,
**Then** the Postgres upsert completes successfully before `add_route` is called.

**Given** a database made to fail,
**When** a create is attempted,
**Then** the call reports no success,
**And** the routing table is unchanged.

**Given** that same failed create,
**When** `GetGatewayStatus` is called on that pod,
**Then** no route exists for that local IP.

**Given** the create and delete handlers side by side,
**When** their write ordering is compared,
**Then** both persist before they cache, so the "persist first, cache second" claim is true of the whole API rather than half of it.

### Story 1.4: Give every route an owner and scope writes to it

As the gateway's administrator,
I want each route to record which `VpnTunnel` owns it,
So that two resources declaring one local IP cannot upsert over each other, and a delete in one namespace cannot destroy another namespace's converged tunnel.

*Realizes FR-34, governed by AD-13. Decision 2026-10-04: an empty `owner` is rejected, not treated as an owner.*

**Acceptance Criteria:**

**Given** the schema seeded through the Postgres image's init hooks,
**When** the seed is applied,
**Then** `vpn_routes` carries an `owner` column holding `<namespace>/<name>`,
**And** it is `TEXT NOT NULL`, not `VARCHAR(255)` like the columns beside it. `<namespace>/<name>` can be 317 characters (63 + 1 + 253), and a narrower column would turn a valid resource into a permanent `INVALID_ARGUMENT` that only delete-and-recreate fixes (AD-15),
**And** it is added to the seed, not introduced as a `sqlx migrate` migration.

**Given** the schema exists in two copies today — `migrations/01_init_routing_table.sql`, which local development applies with `make migrate`, and `k8s/12-configmap-initdb.yaml`, which seeds the cluster's Postgres,
**When** the `owner` column is added,
**Then** both copies change in the same commit and define identical columns,
**And** `.sqlx/` is regenerated against a database that has the new column.

**Given** existing databases built from the old schema — the local compose volume, on which sqlx has recorded a checksum for migration `01`, and the kind Postgres PVC, which init hooks never re-seed,
**When** this story lands,
**Then** the reset for each one is stated, and reachable through the Makefile or written in the story's completion notes (drop and recreate the local volume, delete the kind PVC and redeploy),
**And** nobody is left facing a sqlx checksum mismatch or a Gateway querying a column that doesn't exist.

**Given** `proto/gateway.proto`,
**When** `TunnelRequest` and `DeleteTunnelRequest` gain an `owner` field,
**Then** the regenerated artefacts land in the same commit as the `.proto` edit.

**Given** a create or delete request whose `owner` is empty, which is the proto3 default every caller written before this story sends,
**When** it reaches the Gateway,
**Then** it returns `INVALID_ARGUMENT` before any write,
**And** no row can ever be unowned, because AD-13 gives every row exactly one owner.

**Given** `examples/smoke_client.rs`, whose request literals stop compiling once the messages gain a field,
**When** this story lands,
**Then** the smoke client sends a fixed owner on its create and on both deletes,
**And** `make smoke` passes, because Stories 5.2 and 5.3 gate on it.

**Given** no row for the local IP, or a row already owned by the caller,
**When** `CreateVpnTunnel` is called,
**Then** the row is created or updated.

**Given** a row owned by a different `VpnTunnel`,
**When** `CreateVpnTunnel` is called for that local IP,
**Then** the call returns `FAILED_PRECONDITION`,
**And** nothing in the database or the routing table changes.

**Given** a row owned by a different `VpnTunnel`,
**When** `DeleteVpnTunnel` is called for that local IP,
**Then** the row is untouched,
**And** the call returns `success=true, existed=false`.

**Given** a caller that owns no row for the local IP,
**When** `DeleteVpnTunnel` is called,
**Then** it returns `success=true, existed=false` — FR-2's idempotency is unchanged and a retrying reconcile still depends on it.

### Story 1.5: Return one failure taxonomy from every write path

As the operator's author,
I want the same underlying failure to produce the same gRPC status code everywhere,
So that retry policy can be decided once instead of being inferred from which RPC happened to fail.

*Realizes FR-33, governed by AD-3 and AD-15. Amends the observable behaviour of FR-1 and FR-2 without reopening their Delivered status.*

**Acceptance Criteria:**

**Given** an unreachable database,
**When** `CreateVpnTunnel` or `DeleteVpnTunnel` is called,
**Then** both return `UNAVAILABLE`,
**And** neither returns `INTERNAL`, which is what both return today.

**Given** a Postgres constraint violation — a `tunnel_id` longer than the column, a type error, a check failure,
**When** it propagates out of the store,
**Then** the RPC returns `INVALID_ARGUMENT`, never `INTERNAL`,
**And** it is treated as permanent, because retrying cannot fix it and a reconcile loop that retries it spins forever.

**Given** invalid input,
**When** validation runs,
**Then** `INVALID_ARGUMENT` is returned before any write is attempted.

**Given** the ownership conflict established in Story 1.4,
**When** it occurs,
**Then** the code is `FAILED_PRECONDITION`.

**Given** a genuinely unexpected failure,
**When** it is mapped,
**Then** and only then is `INTERNAL` returned.

**Given** the mapping itself,
**When** the code is inspected,
**Then** it lives at the store boundary in exactly one place,
**And** an RPC added later inherits the taxonomy without restating it.

**Given** a database that accepts connections slowly or never,
**When** a call waits on the pool,
**Then** an acquire timeout set explicitly in `src/main.rs` expires and the call returns `UNAVAILABLE` (sqlx defaults to 30s and the pool sets none today),
**And** that timeout is shorter than Story 3.1's default call deadline, so the operator sees `UNAVAILABLE` from the Gateway instead of hitting its own `DEADLINE_EXCEEDED` (Story 4.4 depends on this).

**Given** the validation bounds the gateway enforces — `local_ip` is IPv4, `tunnel_id` and `remote_endpoint` are bounded at their column widths, `remote_endpoint` is `host:port`,
**When** they are settled here,
**Then** they are recorded explicitly so Story 2.3's CRD markers can mirror the same numbers,
**And** the two validators cannot disagree (AD-15).

### Story 1.6: Read actual state from durable storage with `ListRoutes`

As the operator's author,
I want to read every Tunnel the Gateway owns from the durable record,
So that the reconciler's view of actual state does not depend on which pod answered or whether that pod has hydrated.

*Realizes FR-3, governed by AD-1 and AD-12.*

**Acceptance Criteria:**

**Given** `proto/gateway.proto`,
**When** `ListRoutes` is added,
**Then** it returns `repeated Route`, a new message with exactly three fields — `local_ip`, `tunnel_id`, `remote_endpoint` — keyed on `local_ip` *(Story 1.9 adds `owner` as a fourth field)*,
**And** the shipped `RouteDetails` message is left unmodified, because it is keyed `destination_ip` and belongs to `GetGatewayStatus`.

**Given** the bookkeeping columns `id` and `created_at`,
**When** a `Route` is serialized,
**Then** neither appears on the wire *(`owner` was excluded here as built; Story 1.9 puts it on the wire, 2026-10-07)*, so a whole-message comparison cannot make an already-correct Tunnel look like drift.

**Given** rows in `vpn_routes`,
**When** `ListRoutes` is called,
**Then** every row is returned, read through the store,
**And** the routing table is neither read nor mutated — a read RPC never changes state.

**Given** two Gateway pods and a Tunnel created through pod A,
**When** `ListRoutes` is served by pod B,
**Then** that Tunnel is returned.

**Given** a pod that has never successfully hydrated,
**When** `ListRoutes` is called on it,
**Then** it still returns every row.

**Given** an unreachable database,
**When** `ListRoutes` is called,
**Then** it returns `UNAVAILABLE` through the Story 1.5 mapping,
**And** the caller can distinguish this from "no Tunnels exist", which would otherwise read as total drift and trigger a storm of re-creates.

**Given** a database that is reachable and holds no rows,
**When** `ListRoutes` is called,
**Then** it returns an empty list and OK — an error is never rendered as an empty result.

**Given** a host shell and a running Gateway, and a Gateway that serves no gRPC reflection,
**When** a person needs to see actual state,
**Then** a small example client next to `examples/smoke_client.rs`, run through a Makefile target, prints every route `ListRoutes` returns,
**And** Stories 3.2, 4.1 and 7.2 use this client rather than inventing their own.

### Story 1.7: Bring `AGENTS.md` in line with the code E1 leaves behind

As the next agent or human to open this repository,
I want the written rules to match the code,
So that instructions do not actively mislead whoever builds next.

*The spine says `AGENTS.md` "must be refreshed when E1 lands". Its false rules about `log` and the 1.36 cutover have already been corrected (as of 2026-10-04). It now describes `println!` as today's practice and AD-9 as the plan, and that description stops being true once Story 1.1 lands.*

**Acceptance Criteria:**

**Given** the logging entry, which today says `println!` is used throughout and `tracing` is unused,
**When** `AGENTS.md` is updated after Story 1.1,
**Then** it says `tracing` with `tracing-subscriber` is the only logging path, no `println!` remains, and none may be added,
**And** the sentence about how the earlier version was wrong is removed, since it no longer helps anyone reading.

**Given** the SQL boundary established in Story 1.2,
**When** the file is updated,
**Then** AD-2 is recorded as a rule: no SQL outside `src/store/`.

**Given** the `make check` / `make check-sqlx` split from Story 1.2,
**When** the *Running and verifying* section is updated,
**Then** it says which check needs Postgres and which does not.

**Given** the pitfall about the kind node image pinned at `v1.33.1`,
**When** the file is updated,
**Then** the pin stays in place because the host fix has not been applied,
**And** a note records that Stories 5.1 (the host fix) and 5.3 (the pin move) retire it.

*Stories 1.8–1.10 were added 2026-10-07 by the Epic 1 retrospective and `sprint-change-proposal-2026-10-07.md`; they land before Epic 2.*

### Story 1.8: Bound what a tunnel identifier and an owner may contain

As the operator's author,
I want the gateway to refuse identifiers that Postgres cannot store or a log cannot show safely,
So that a bad value fails at `kubectl apply` or at validation, never inside the database, and the CRD can mirror one exact rule.

*Realizes AD-15 as amended 2026-10-07. Closes retro findings F-9, F-10, F-11, F-12 and the comment fixes F-2/F-8, and deferrals 1.5 #2 and 1.1 #5.*

**Acceptance Criteria:**

**Given** a create whose `tunnel_id` matches `^[A-Za-z0-9][A-Za-z0-9._-]*$` and is 1 to 255 characters,
**When** it is validated,
**Then** it passes,
**And** any other `tunnel_id` (one containing NUL, CR, LF, a space or a non-ASCII character, or starting with `.`, `_` or `-`) returns `INVALID_ARGUMENT` before any write, with a message that does not echo the value.

**Given** a create or delete whose `owner` contains a control character (U+0000–U+001F, U+007F–U+009F) or is longer than 317 characters,
**When** it is validated,
**Then** it returns `INVALID_ARGUMENT` before any write,
**And** its format is otherwise unchecked (the 2026-10-04 decision stands), and the column stays `TEXT`.

**Given** rows stored before this story whose `tunnel_id` breaks the new pattern,
**When** the gateway hydrates or serves `ListRoutes`,
**Then** they are still loaded and listed, because the bound applies to writes only.

**Given** the five-place rule in `src/services/validation.rs`,
**When** the bounds change,
**Then** the consts, the module doc, the `proto/gateway.proto` comments and the Story 2.3 AC all state the same pattern and lengths, and the `owner` bound is stated in the gateway and the proto only, because no CRD field carries it.

**Given** the two schema copies,
**When** `make test` runs,
**Then** an offline test asserts that `migrations/01_init_routing_table.sql` and `k8s/12-configmap-initdb.yaml` define the same `vpn_routes` columns, and that the `VARCHAR` widths equal the validation consts.

**Given** a create or delete with `local_ip` `"10.0.0.999"`, `"010.0.0.5"` or `" 10.0.0.5"`,
**When** it reaches the handler,
**Then** it returns `INVALID_ARGUMENT`, not `UNAVAILABLE`, and the routing table is unchanged,
**And** a request invalid in every field reports the `local_ip` failure first.

**Given** a store that cannot reach Postgres,
**When** a call fails,
**Then** a test asserts it fails `Unavailable` within 2 seconds, so a dropped acquire timeout fails `make test` instead of only slowing it.

**Given** the retro's comment findings,
**When** this story lands,
**Then** the `kind/cluster.yaml` header names `examples/list_routes.rs`, and `AGENTS.md` says `hydrate()` lives in `src/services/gateway.rs`.

### Story 1.9: Expose route ownership on `ListRoutes`

As the operator's author,
I want every listed route to carry its owner,
So that a reconciler tells "converged and mine" from "matches, but someone else's" without making a write.

*Realizes FR-3 and FR-34 as amended 2026-10-07, governed by AD-12 and AD-13 as amended. Closes retro finding F-5, and with it the orphan-visibility finding.*

**Acceptance Criteria:**

**Given** `proto/gateway.proto`,
**When** this story lands,
**Then** `Route` gains `string owner = 4`, and `id` and `created_at` stay off the wire,
**And** the proto comment defines "already correct" as `owner` equal to the caller's and `tunnel_id` and `remote_endpoint` equal by exact string, and calls a matching row under another owner a conflict.

**Given** `Store::list_routes`,
**When** it reads `vpn_routes`,
**Then** it selects `owner`, and the regenerated `.sqlx/` lands in the same commit,
**And** `ListRoutes` copies it verbatim. Hydration and the `RoutingTable` are unchanged: the cache does not hold owners.

**Given** the proto's size note,
**When** it is restated,
**Then** it gives the ceiling at maximum widths in bytes: `tunnel_id` 255 ASCII, `remote_endpoint` 255 ASCII, `owner` 317 characters of up to 4 bytes each. That is roughly 2,300 routes under tonic's 4 MiB default.

**Given** `make routes`,
**When** it prints actual state,
**Then** each line shows the owner, escaped like the other fields.

**Given** `make smoke`,
**When** it runs,
**Then** it asserts that `ListRoutes` shows the smoke client's owner on its own row, and still shows that owner after the refused foreign-owner create.

### Story 1.10: Prove the store's SQL against a real database

As the gateway's maintainer,
I want the owner-guarded upsert, the owner-scoped delete and `ListRoutes` tested automatically against Postgres,
So that a regression in the SQL fails a check instead of waiting for someone to run `make smoke`.

*Closes retro finding F-20: deferrals 1.2 #1-3, 1.3 #8, 1.4 #5, 1.5 #17 and 1.6 #12.*

**Acceptance Criteria:**

**Given** a running Postgres from `make db-up`,
**When** `make test-db` runs,
**Then** `#[sqlx::test]` tests run, each on a fresh database built from `migrations/`,
**And** `make test` stays offline and does not run them.

**Given** the handlers over a real store,
**When** the tests run,
**Then** they prove each of the following:
- a create stores the row and caches the route;
- an owner's second create updates its row and cache;
- a foreign-owner create returns `FAILED_PRECONDITION` and leaves the row, its owner and the cache unchanged;
- a foreign-owner delete returns `existed=false` and keeps the cached route;
- an owner's delete removes the row and clears the cache;
- concurrent creates by two owners for one `local_ip`, over many rounds, always produce exactly one winner;
- `ListRoutes` returns the database's rows, with owners, when the cache disagrees;
- an empty table returns OK and an empty list.

**Given** `AGENTS.md`,
**When** this story lands,
**Then** it lists `make test-db` as a fourth check that needs Postgres, and drops the claim that no unit test reaches a working database.

**Given** `deferred-work.md`,
**When** this story lands,
**Then** the entries it closes are marked resolved.

---

## Epic 2: The `VpnTunnel` API

A user can declare a Tunnel as a Kubernetes resource and have the API server reject it if it is wrong, before any controller sees it. `kubectl get vpntunnels` renders a useful table. Nothing reconciles yet — and that is a coherent stopping point, because this API is the contract everything downstream is written against.

*Go · Phase B · depends on E1 (Stories 1.5, 1.8) · FR-13, FR-14 · AD-10, AD-15, conventions: Naming, Identity, Conditions*

### Story 2.1: Install the Go toolchain on this host

**⚠️ Performed by Lilian. No agent executes any step of this story.**

As the builder,
I want Go 1.27.1 present on this machine,
So that Phase B can start at all — controller-runtime requires >= 1.26 and the spine records that Go is not installed here.

**Acceptance Criteria:**

**Given** a host where `go version` currently fails,
**When** Go 1.27.1 has been installed,
**Then** `go version` reports `go1.27.1`,
**And** it satisfies controller-runtime v0.25.1's minimum of 1.26.

**Given** the 4 GB resource ceiling (NFR6),
**When** the toolchain and its module cache are in place,
**Then** the host still has headroom to run a kind cluster,
**And** the standing advice to stop `rust-analyzer` — which alone held 1.3 GB — before cluster work is recorded somewhere a future session will read it.

**Given** this story's nature as a host change,
**When** it is picked up,
**Then** no agent performs any part of it,
**And** the story is closed by Lilian confirming the verification commands above.

### Story 2.2: Scaffold the operator as an independent Go module

As the builder,
I want a `kubebuilder` project at `operator/`,
So that the orchestration layer and the data-plane service are visibly separate programs in separate languages, which is the real architecture and the thing a reviewer is meant to see.

*Enabling work with no FR of its own — notes milestone M6, PRD §8.1.*

**Acceptance Criteria:**

**Given** the repository root,
**When** `kubebuilder` v4.16.0 initialises a project at `operator/` with domain `net.lilianmrt.dev`,
**Then** `operator/go.mod` exists as a module independent of the Rust crate,
**And** `cargo build` at the root is unaffected by its presence.

**Given** the pinned stack,
**When** dependencies resolve,
**Then** controller-runtime is v0.25.1 and `k8s.io/api` / `client-go` are v0.37.0.

**Given** AD-4's rule that `go build` and `go test` must never require `protoc`,
**When** the scaffolded module is built on a machine without `protoc`,
**Then** the build succeeds.

**Given** AD-8's rule that the Makefile is the human interface,
**When** the operator's build, test and manifest targets are wired,
**Then** they are reachable from the root Makefile,
**And** a reader who opens it sees one interface covering both languages.

**Given** the package layout the spine fixes,
**When** the scaffold is laid down,
**Then** it provides for `api/v1alpha1`, `internal/controller` and `internal/gateway`,
**And** no package outside `internal/gateway` is positioned to import gRPC (AD-5).

### Story 2.3: Declare a Tunnel as a `VpnTunnel` resource

As the gateway's administrator,
I want to declare a Tunnel as a Kubernetes resource that the API server validates,
So that a malformed tunnel is rejected at `kubectl apply` and never reaches the Operator or the Gateway.

*Realizes FR-13, governed by AD-10 and AD-15.*

**Acceptance Criteria:**

**Given** the API group,
**When** the type is defined,
**Then** it is group `net.lilianmrt.dev`, version `v1alpha1`, Kind `VpnTunnel`, namespace-scoped.

**Given** `spec`,
**When** a user writes a manifest,
**Then** it carries `gatewayRef` (name and namespace), `localIP`, `tunnelID` and `remoteEndpoint`,
**And** `gatewayRef` carries **no port field**, because the port is a package constant in `internal/gateway` (AD-10).

**Given** Go's initialism convention,
**When** the JSON tags are written,
**Then** they read `localIP`, `tunnelID` and `remoteEndpoint` — not `localIp`.

**Given** an invalid `localIP` such as `not-an-ip` or `10.0.0.999`,
**When** `kubectl apply` is run,
**Then** the API server rejects it through a validation marker,
**And** nothing reaches the Operator.

**Given** a `remoteEndpoint` that is not `host:port`,
**When** `kubectl apply` is run,
**Then** it is rejected by the API server.

**Given** the bounds Stories 1.5 and 1.8 recorded for the gateway,
**When** the markers are written,
**Then** `tunnelID` and `remoteEndpoint` are bounded at the same column widths the gateway enforces,
**And** `tunnelID` carries the pattern `^[A-Za-z0-9][A-Za-z0-9._-]*$`, so a NUL, a newline or a space is rejected at `kubectl apply`,
**And** the two validators cannot disagree, because a value that passes here and fails there is only fixable by delete-and-recreate (AD-15).

**Given** an existing `VpnTunnel`,
**When** a user edits `spec.localIP`,
**Then** the update is rejected — `localIP` is the unique key and the Gateway API has no rename operation.

**Given** the closed decision on scope,
**When** the type is defined,
**Then** it carries no field without a counterpart in `vpn_routes`,
**And** in particular no `appliedRoutes`.

**Given** the CRD manifest generated by `controller-gen`,
**When** it is produced,
**Then** it lands under the scaffold's `config/crd/bases` and is applied with `kubectl`,
**And** Story 5.2 relocates it to `charts/netgw/crds/` when the chart exists.

### Story 2.4: See tunnel state from `kubectl get`

As the gateway's administrator,
I want to see whether a Tunnel has converged without describing the resource,
So that the state of every declared tunnel is one command away.

*Realizes FR-14.*

**Acceptance Criteria:**

**Given** the `VpnTunnel` type,
**When** `status` is defined,
**Then** it is a subresource,
**And** a status write cannot conflict with a user's concurrent spec edit.

**Given** several `VpnTunnel` resources,
**When** `kubectl get vpntunnels` is run,
**Then** the table shows printer columns for local IP, tunnel ID, Ready and age.

**Given** `status.conditions`,
**When** it is defined,
**Then** it is `[]metav1.Condition` in the standard shape, written through `meta.SetStatusCondition`,
**And** the condition type is `Ready`.

**Given** the reasons the Operator will need,
**When** they are declared as constants,
**Then** they include at least `Converged`, `GatewayUnreachable`, `InvalidSpec`, `OwnedByAnother` and `Deleting`,
**And** no reason is ever produced by string-matching a gRPC message.

**Given** `status.observedGeneration`,
**When** the type is defined,
**Then** the field exists and is documented as advancing only after a successful converge,
**And** it is left unset by this story, because nothing reconciles until Epic 3.

**Given** a freshly applied resource that no controller has touched,
**When** `kubectl get vpntunnels` is run,
**Then** the table renders without error and the Ready column shows an unset state rather than a false positive.

---

## Epic 3: Converge a tunnel

`kubectl apply` makes a tunnel real. The Operator reads actual state fresh, diffs it against the spec, creates what is missing, writes honest status, and says why when it cannot — without ever blocking a worker or caching what it saw. This is the first epic a user can demonstrate end to end.

*Go · Phase B · depends on E1, E2 · realizes UJ-1 · FR-21, FR-15, FR-32, FR-16, FR-19, FR-20, FR-26 · AD-3, AD-4, AD-5, AD-6, AD-7, AD-10, AD-12, AD-14, AD-15*

**Decisions 2026-10-04, binding on this epic:**
- **Address resolution is injectable, and a dev-only flag exposes it.** AD-10's `<name>.<namespace>.svc.cluster.local:50051` stays the one in-cluster DNS construction and the default. `internal/gateway` accepts an injected resolver, which FR-25's `testcontainers-go` test needs regardless, and a `--gateway-address` flag lets an out-of-cluster operator reach the kind Gateway on `127.0.0.1:50051`. That is what makes UJ-1 demonstrable here rather than in E6.
- **This epic's acceptance criteria are verified with controller-runtime's fake client and the counting gateway fake**, not `envtest`. No envtest binaries land against the 4 GB ceiling until E4, which builds the FR-24 suite on top.
- **Known gap, accepted:** the finalizer is FR-18 in E4, so until it lands, deleting a `VpnTunnel` leaves its row in `vpn_routes`. AD-14 already treats an orphan row as a legitimate steady state.
- **Superseded 2026-10-07 (Epic 1 retro F-5):** the 2026-10-04 known limit — `ListRoutes` carrying no owner — is removed by Story 1.9. A row that matches the spec but is held by another owner is now a conflict seen on read: `Converged` means this resource owns the declared route.

### Story 3.1: Reach a Gateway through one client boundary

As the operator's author,
I want every Gateway call to go through one package that owns addressing, timeouts and error classification,
So that the reconciler can decide what to do about a failure without knowing it came over gRPC.

*Realizes FR-21, governed by AD-3, AD-4, AD-5, AD-6, AD-10 and AD-12. Decision 2026-10-04: address resolution can be injected, and a dev-only flag exposes it.*

**Acceptance Criteria:**

**Given** `proto/gateway.proto`,
**When** `make proto` runs,
**Then** Go stubs are generated into `operator/internal/gatewaypb/` and committed,
**And** `go build ./...` and `go test ./...` in `operator/` succeed on a machine without `protoc`.

**Given** committed stubs that no longer match the `.proto`,
**When** `make check` runs,
**Then** it regenerates them and fails on `git diff --exit-code`,
**And** it does so without a database, because `.sqlx/` is checked separately by `make check-sqlx` (Story 1.2).

**Given** a host with `protoc` but without `protoc-gen-go` or `protoc-gen-go-grpc`, which Story 2.1 does not install,
**When** `make proto` or `make check` runs,
**Then** the Makefile installs both plugins at pinned versions into a project-local `bin/`, the way the scaffold handles `controller-gen`,
**And** generation never depends on a plugin already installed on the host.

**Given** a `gatewayRef` of `{name: gateway, namespace: netgw}`,
**When** the default resolver runs,
**Then** the address is `gateway.netgw.svc.cluster.local:50051`, with the port taken from a package constant,
**And** no Kubernetes API read is made.

**Given** the operator started with the dev flag `--gateway-address=127.0.0.1:50051`,
**When** any `gatewayRef` is resolved,
**Then** that address is used in its place,
**And** the flag's help text and the code both say it exists only for running out of cluster, with the AD-10 DNS rule as the default.

**Given** two resources naming the same `gatewayRef`,
**When** both reconcile,
**Then** one connection made with `grpc.NewClient` serves both and is reused for the life of the process,
**And** nothing calls `grpc.Dial` or `WithBlock`, so the operator starts whether or not the Gateway is up.

**Given** any `List`, `Create` or `Delete` call,
**When** it is made,
**Then** it carries a deadline whose default value is stated in the package and is longer than the Gateway's pool acquire timeout from Story 1.5,
**And** a Gateway that accepts the connection and never answers produces a deadline error instead of a blocked worker.

**Given** a gRPC status from the Gateway,
**When** it is classified,
**Then** `UNAVAILABLE`, `DEADLINE_EXCEEDED` and `INTERNAL` are retryable, `INVALID_ARGUMENT` is permanent, and `FAILED_PRECONDITION` is a conflict,
**And** any code not listed here is retryable, so the loop never stops trying without saying why,
**And** classification reads the status code and never the message text,
**And** the typed error keeps the original code so it can be logged and shown in status.

**Given** the package's interface,
**When** `Create` or `Delete` is called,
**Then** it takes the spec fields plus an owner of the form `<namespace>/<name>`,
**And** `List` returns the package's own route type, never a `gatewaypb` type,
**And** the mapping from `localIP` to `local_ip` (and the other field names) exists only in this package.

**Given** `operator/internal/controller`,
**When** its imports are checked,
**Then** neither `google.golang.org/grpc` nor `internal/gatewaypb` appears.

**Given** the counting fake shipped alongside the package,
**When** a test uses it,
**Then** it records the number of calls per method, keeps routes in memory, and can be told to return each error class.

### Story 3.2: Converge a tunnel from whatever the Gateway reports

As the gateway's administrator,
I want `kubectl apply` of a `VpnTunnel` to make that tunnel exist on its Gateway,
So that I declare tunnels instead of calling RPCs.

*Realizes FR-15, FR-32, FR-16 and the namespace-and-name half of FR-26. Governed by AD-1, AD-5, AD-6, AD-7, AD-12 and AD-14. Until Story 4.2 adds the finalizer, deleting a `VpnTunnel` leaves its row behind. AD-14 treats that row as a legitimate steady state, not a defect.*

**Acceptance Criteria:**

**Given** the controller is registered,
**When** a `VpnTunnel` is created, updated or deleted,
**Then** a Reconcile runs for that resource and no other.

**Given** a request for a `VpnTunnel` that no longer exists,
**When** Reconcile runs,
**Then** it returns no error,
**And** the counting fake records zero Gateway calls.

**Given** any pass,
**When** Reconcile starts,
**Then** it calls `List` before deciding anything,
**And** two consecutive passes record two `List` calls.

**Given** the reconciler and its package,
**When** they are inspected,
**Then** no struct field, package variable or cache holds Gateway-derived data beyond one Reconcile,
**And** the connection cached in `internal/gateway` is exempt, because it is transport and not state (AD-6).

**Given** no route for `spec.localIP`,
**When** Reconcile runs,
**Then** exactly one `Create` is made, carrying the owner `<namespace>/<name>`.

**Given** a route whose `tunnel_id` or `remote_endpoint` differs from the spec,
**When** Reconcile runs,
**Then** exactly one `Create` is made, which upserts.

**Given** a route whose `owner` is this resource's `<namespace>/<name>` and which matches the spec on `tunnel_id` and `remote_endpoint` by exact string comparison,
**When** Reconcile runs,
**Then** the fake records zero `Create` and zero `Delete` calls.

**Given** a route for `spec.localIP` whose `owner` is a different resource, whatever its `tunnel_id` and `remote_endpoint`,
**When** Reconcile runs,
**Then** the fake records zero `Create` and zero `Delete` calls,
**And** the pass reports the conflict through Story 3.4's `OwnedByAnother` reason.

**Given** `List` returns routes for other local IPs, including ones no `VpnTunnel` declares,
**When** Reconcile runs,
**Then** it neither creates nor deletes anything for them, because a sibling or orphan row is outside the loop's remit (AD-14).

**Given** each of the three starting states (absent, present but wrong, already correct),
**When** one Reconcile runs,
**Then** a following `List` shows the tunnel correct.

**Given** any line logged during a reconcile,
**When** it is emitted,
**Then** it carries the resource's namespace and name, through the logger controller-runtime puts in the context.

**Given** the gateway deployed on kind and the operator started by a Makefile target that runs it against the current kubeconfig with `--gateway-address=127.0.0.1:50051`,
**When** a `VpnTunnel` manifest is applied,
**Then** the row appears in the route listing from Story 1.6's client, which shows UJ-1 working end to end before the operator is deployed in E6.

### Story 3.3: Report convergence honestly

As the gateway's administrator,
I want a `VpnTunnel` to show Ready only when its current spec has converged,
So that I can tell current status from stale status.

*Realizes FR-19's success path.*

**Acceptance Criteria:**

**Given** a pass that ends with the tunnel correct, whether it was just created or already matched,
**When** status is written,
**Then** `Ready` is `True` with reason `Converged`, set through `meta.SetStatusCondition`,
**And** `status.observedGeneration` equals `metadata.generation`.

**Given** a converged resource at generation N whose spec is then edited to generation N+1,
**When** no pass has yet succeeded at N+1,
**Then** `status.observedGeneration` stays at N.

**Given** a pass that fails at any point,
**When** it returns,
**Then** `observedGeneration` has not moved.

**Given** status that already matches what the pass would write,
**When** Reconcile runs again,
**Then** no status update is sent, so a converged resource does not trigger itself in a loop.

**Given** any status write,
**When** it is made,
**Then** it goes through the status subresource and never through an update of the whole object.

**Given** controller-runtime's fake client, which does not manage `generation`,
**When** these criteria are tested,
**Then** the tests set `generation` explicitly, and the envtest suite in E4 (FR-24) repeats the generation-lag check against a real API server.

### Story 3.4: Fail legibly

As the gateway's administrator,
I want every failure to show its reason in `kubectl get` and to be retried only when retrying can fix it,
So that a Gateway outage heals by itself and a bad spec does not spin forever.

*Realizes FR-19's failure reasons, FR-20, and the gRPC-code half of FR-26. Governed by AD-3, AD-15, NFR3 and NFR7.*

**Acceptance Criteria:**

**Given** a retryable error from `List` or `Create`,
**When** Reconcile handles it,
**Then** `Ready` is `False` with reason `GatewayUnreachable` and the gRPC code in the message,
**And** Reconcile returns the error, so controller-runtime applies exponential backoff.

**Given** a Gateway that stays unreachable,
**When** the operator keeps running,
**Then** its logs show retries with growing gaps between them, not a tight loop,
**And** no worker is ever blocked waiting.

**Given** a permanent error (`INVALID_ARGUMENT`),
**When** Reconcile handles it,
**Then** `Ready` is `False` with reason `InvalidSpec`,
**And** Reconcile returns no error and asks for no backoff retry, because retrying cannot fix the spec and the remedy is to delete and recreate it (AD-15),
**And** from Story 4.1 onward the resource is still re-checked at the resync interval, which is a re-check and not a retry.

**Given** a conflict — a `FAILED_PRECONDITION` from `Create`, or a route listed under another owner (Story 3.2) —
**When** Reconcile handles it,
**Then** `Ready` is `False` with reason `OwnedByAnother` and a message naming the conflict,
**And** nothing is retried with backoff and nothing changes at the Gateway.

**Given** a Gateway that comes back after `GatewayUnreachable`,
**When** the next backoff retry runs,
**Then** `Ready` returns to `True/Converged` with no human action.

**Given** any condition reason,
**When** it is chosen,
**Then** it comes from the typed error class in Story 3.1 and never from matching message text.

**Given** a failed Gateway call,
**When** it is logged,
**Then** the gRPC code is a structured key on the line, not only text inside the message.

**Given** `operator/internal/controller`,
**When** it is searched for `time.Sleep`,
**Then** there are no matches.

---

## Epic 4: Drift and deletion

The loop becomes a control plane rather than an apply-once tool: a route deleted behind its back comes back within 30 seconds with no event involved, and deleting the resource actually deletes the tunnel instead of orphaning it. Verified against a real API server and a real Gateway.

*Go · Phase B · depends on E3 · realizes UJ-2, UJ-3 · FR-17, FR-18, FR-24, FR-25 · AD-4, AD-5, AD-6, AD-13, AD-14*

**Guardrail:** AD-14 — no reaper. SM-5 is satisfied by finalizers removing what they created, never by sweeping `ListRoutes` for unmatched rows.

**Decision 2026-10-04:** FR-17's "every Reconcile returns a requeue-after" applies to permanent and conflict outcomes as well. They are re-checked at the resync interval, not retried under backoff, so an `OwnedByAnother` resource can take over its local IP once the rival's row is gone. Story 4.1 proves this by removing the row directly. Story 4.2 proves the path through `kubectl delete`, because the rival's row only goes away through its finalizer. Story 3.4's wording was amended to match.

### Story 4.1: Correct drift on a timer

As the gateway's administrator,
I want every tunnel re-checked on a fixed schedule even when nothing in the cluster changes,
So that a route removed behind the operator's back comes back without anyone noticing it was gone.

*Realizes FR-17 and UJ-2. Governed by AD-6 and AD-14. Amends Story 3.4: permanent and conflict outcomes are re-checked at the resync interval instead of never.*

**Acceptance Criteria:**

**Given** a pass that converges, or that ends in `InvalidSpec` or `OwnedByAnother`,
**When** Reconcile returns,
**Then** it returns no error and a `RequeueAfter` equal to the resync interval.

**Given** a pass that returns a retryable error,
**When** Reconcile returns,
**Then** controller-runtime's backoff governs the retry, and the resync interval does not shorten or replace it.

**Given** the operator's flags,
**When** it starts with no resync flag,
**Then** the interval is 30 seconds,
**And** a flag in the controller-runtime idiom changes it (E6 surfaces that flag as a Helm value).

**Given** a converged tunnel deleted directly through the Gateway API, with no cluster event,
**When** the next resync fires,
**Then** the tunnel is re-created within 30 seconds,
**And** the resource stays `Ready=True/Converged` throughout, so no status update is sent.

**Given** a resource in `OwnedByAnother` whose rival's row is then removed with the owner-carrying delete client below, passing the rival's `<namespace>/<name>`,
**When** the next resync fires,
**Then** the resource claims the now-free local IP and reaches `Ready=True/Converged`,
**And** this story does not rely on deleting the rival `VpnTunnel`, because until Story 4.2 adds the finalizer, that leaves the rival's row in place.

**Given** a resync pass,
**When** it runs,
**Then** it issues its own `List` and looks only at its own `localIP`,
**And** nothing in this story enumerates or deletes rows that no `VpnTunnel` declares (AD-14: no reaper).

**Given** a host shell and a running Gateway,
**When** a person needs to delete one route behind the operator's back,
**Then** a small example client run through a Makefile target calls `DeleteVpnTunnel` with a local IP **and the owning resource's `<namespace>/<name>`**,
**And** it requires the owner, because AD-13 makes a delete without the matching owner return `existed=false` and leave the row in place.

**Given** the operator running out of cluster as in Story 3.2,
**When** the UJ-2 sequence is performed by hand (apply, wait for Ready, delete with that client, then watch the Story 1.6 listing),
**Then** restoration is visible within one interval.

### Story 4.2: Remove tunnels through a finalizer

As the gateway's administrator,
I want `kubectl delete vpntunnel` to remove the tunnel from the Gateway before the resource disappears,
So that deleting the declaration deletes what it declared.

*Realizes FR-18 and UJ-3. Governed by AD-13 and AD-14. Closes the orphan gap Epic 3 accepted.*

**Acceptance Criteria:**

**Given** a resource without the finalizer,
**When** Reconcile runs,
**Then** `net.lilianmrt.dev/tunnel-cleanup` is added and persisted **before** any `Create` call,
**And** a crash between the two cannot leave a tunnel with no finalizer guarding it.

**Given** a resource that converged under Epic 3 and has no finalizer,
**When** its next pass runs,
**Then** the finalizer is added, with no other change.

**Given** a resource with `deletionTimestamp` set,
**When** Reconcile runs,
**Then** it calls `Delete` with its `localIP` and owner, removes the finalizer once that succeeds, and returns,
**And** it makes no `Create` call.

**Given** a tunnel already gone from the Gateway, or a row owned by someone else,
**When** the delete runs,
**Then** the Gateway returns `success=true, existed=false`, and the finalizer is removed exactly as on a normal delete.

**Given** a retryable error from `Delete`,
**When** Reconcile handles it,
**Then** `Ready` is `False` with reason `Deleting` and the gRPC code in the message,
**And** the error is returned for backoff, the resource stays `Terminating`, and it completes on its own once the Gateway returns.

**Given** a permanent error from `Delete`,
**When** Reconcile handles it,
**Then** the finalizer stays in place and `Ready=False/Deleting` gives the code and the reason,
**And** the error is not swallowed so the object gets collected anyway.

**Given** the deletion path,
**When** it is inspected,
**Then** it removes only this resource's own row, and never lists rows or deletes on behalf of other resources (AD-14).

**Given** two `VpnTunnel` resources declaring one local IP, one `Ready=True/Converged` and the other `Ready=False/OwnedByAnother`,
**When** the converged one is removed with `kubectl delete`,
**Then** its finalizer removes its row,
**And** on its next resync the other resource claims the local IP and reaches `Ready=True/Converged`, with no human action.

### Story 4.3: Prove the loop against a real API server

As the operator's author,
I want the controller's named behaviours checked against a real API server,
So that I'm testing status subresources, generations and finalizers as Kubernetes actually implements them, rather than as the fake client imitates them.

*Realizes FR-24. Governed by AD-5. Contributes to SM-5.*

**Acceptance Criteria:**

**Given** an `envtest` API server loaded with the CRD generated in Story 2.3 and a reconciler wired to the counting fake,
**When** the suite runs,
**Then** it covers converge from absent, converge from wrong, no-op on already-correct, finalizer removal on delete, and `observedGeneration` lagging a spec edit until the next successful pass.

**Given** a converged resource,
**When** Reconcile is run repeatedly against it,
**Then** the fake records no further `Create` or `Delete` calls (the idempotency assertion).

**Given** a manifest that breaks Story 2.3's validation markers, such as a non-IPv4 `localIP` or a `localIP` edit,
**When** it is submitted to the `envtest` API server,
**Then** it is rejected, giving those markers their first automated check.

**Given** the suite's end state,
**When** every test has torn down,
**Then** the fake holds no route without a matching `VpnTunnel`.

**Given** a clean machine with no cluster,
**When** the suite is run through a Makefile target,
**Then** it passes, fetching the envtest binaries itself if they are missing,
**And** it runs within the 4 GB ceiling with the kind cluster stopped.

### Story 4.4: Prove the loop against a real Gateway

As the operator's author,
I want the client and the reconciler exercised against the real Gateway image and a real Postgres,
So that the demo path is verified end to end, not only against a fake.

*Realizes FR-25. Governed by AD-4, AD-5 and AD-6; uses the injectable resolver decided for Epic 3.*

**Acceptance Criteria:**

**Given** the Gateway image built by `make image` and Postgres 15 seeded through its init hooks with the Story 1.4 schema, read from `migrations/01_init_routing_table.sql` because Story 5.2 deletes `k8s/`,
**When** the integration suite starts,
**Then** `testcontainers-go` v0.44.0 runs both as containers,
**And** `internal/gateway` reaches the Gateway through the injected resolver at the mapped host port.

**Given** the real gRPC API,
**When** the lifecycle test runs,
**Then** it creates a tunnel, sees it in `ListRoutes`, deletes it, sees it gone, and deletes it again, getting `success=true, existed=false`.

**Given** a tunnel converged by Reconcile, driven directly with the fake Kubernetes client,
**When** it is deleted through the Gateway API, passing the resource's `<namespace>/<name>` as owner as AD-13 requires, and Reconcile runs again,
**Then** the row was really gone before that pass, which the test checks with `ListRoutes`,
**And** the tunnel is restored (the UJ-2 drift case).

**Given** a Postgres container that is stopped, not paused, because a paused database holds connections open until the client's deadline fires and the test would see `DEADLINE_EXCEEDED`,
**When** a create is sent,
**Then** it returns `UNAVAILABLE` within the client's deadline,
**And** `GetGatewayStatus` shows no route for that local IP (the FR-31 regression).

**Given** `go test ./...` with no extra flags,
**When** it is run,
**Then** the integration suite is skipped, kept behind a build tag and its own Makefile target, so the default test run never needs Docker.

**Given** the 4 GB ceiling,
**When** the suite runs,
**Then** it does so with the kind cluster stopped.

---

## Epic 5: Package the Gateway *(PRD E5a)*

One command installs the Gateway, Postgres and the CRD into a clean cluster, and one removes them. The Makefile stays the human interface with Helm underneath it, and `k8s/` disappears.

*depends on E1, E2 · FR-12 · AD-8, AD-11 · conventions: Schema, Secrets, Hermetic builds*

**Also carries:** the WSL cgroup fix (**performed by Lilian**), then moving the node image pin to `v1.37.0` by digest. Story 5.2 does not depend on 5.1, so the chart is not blocked on the host change.

**Decision 2026-10-04:** the schema stays seeded through the Postgres image's init hooks. The migration-Job question raised by PRD §6 and the spine's *Deferred* list is closed: a Job adds hooks, ordering and sqlx bookkeeping for no reader-visible gain, against SM-C1.

### Story 5.1: Put this host on the unified cgroup hierarchy

**⚠️ Performed by Lilian. No agent executes any step of this story.**

As the builder,
I want this WSL2 host to mount cgroup v2 only,
So that Kubernetes 1.35 and later can boot here and the node image no longer has to be pinned to a version that is aging out.

*AD-11 prerequisite, owned by E5. Overrides PRD §6's original deferral.*

**Acceptance Criteria:**

**Given** the Windows-side `%UserProfile%\.wslconfig`,
**When** `kernelCommandLine = cgroup_no_v1=all` is added under `[wsl2]` and `wsl --shutdown` has been run,
**Then** `/proc/cmdline` inside WSL contains `cgroup_no_v1=all`,
**And** `/sys/fs/cgroup/cgroup.controllers` exists,
**And** `mount -t cgroup` lists nothing, so no v1 hierarchy is left.

**Given** Docker after the restart,
**When** `docker info` is run,
**Then** it reports `Cgroup Version: 2`.

**Given** `wsl --shutdown` ends every running WSL session, including any agent session,
**When** this story is scheduled,
**Then** it happens at a natural stopping point with work committed.

**Given** this story's nature as a host change,
**When** it is picked up,
**Then** no agent performs any part of it,
**And** it is closed by Lilian confirming the verification commands above.

### Story 5.2: Install the Gateway, Postgres and the CRD from one chart

As the gateway's administrator,
I want one command to install the whole system into a clean cluster and one to remove it,
So that there is one documented way to deploy, and the Makefile and Helm don't each describe it differently.

*Realizes FR-12, governed by AD-8 and AD-11. Does not depend on Story 5.1.*

**Acceptance Criteria:**

**Given** the manifests in `k8s/`,
**When** they become `charts/netgw/templates/` in a chart with `apiVersion: v2`,
**Then** the `k8s/` directory is removed in the same change,
**And** `helm lint charts/netgw` passes.

**Given** the chart's values,
**When** they are read,
**Then** the Gateway image tag and the Gateway replica count are values, not literals,
**And** the replica count defaults to 1, with a comment that going above 1 needs PRD Q1 settled first (AD-11, Deferred).

**Given** the `VpnTunnel` CRD from Story 2.3,
**When** the operator's manifest generation runs,
**Then** it writes the CRD into `charts/netgw/crds/`,
**And** `make check` fails if the committed CRD is stale, following the AD-4 pattern.

**Given** an empty kind cluster,
**When** `make deploy` runs,
**Then** it first runs `kubectl apply -f charts/netgw/crds/`, then `helm upgrade --install`,
**And** a later change to the CRD reaches the cluster on the next `make deploy`, even though `helm upgrade` never updates `crds/`.

**Given** the installed release,
**When** it is compared with what the raw manifests produced,
**Then** Postgres is a StatefulSet with a PVC behind a headless Service, the Gateway is a Deployment behind NodePort 30051, and `imagePullPolicy` is `IfNotPresent`,
**And** `make smoke` and `make probe` pass against it,
**And** deleting the Gateway pod produces a replacement that hydrates the same tunnels and becomes ready.

**Given** the Postgres init hooks,
**When** a fresh volume is seeded,
**Then** the schema includes Story 1.4's `owner` column, applied through the init-hook ConfigMap and not as a migration.

**Given** the Gateway pod rendered from the chart,
**When** its spec is inspected,
**Then** it carries the full FR-11 posture unchanged: explicit non-root uid, all capabilities dropped, no privilege escalation, `RuntimeDefault` seccomp, read-only root filesystem.

**Given** the Secret and the NetworkPolicies,
**When** they are templated,
**Then** the Secret is still labelled as a deliberately committed dev-only value,
**And** the NetworkPolicies keep their header saying kind's CNI does not enforce them.

**Given** an installed release,
**When** `make undeploy` runs `helm uninstall`,
**Then** no namespaced object remains apart from the Postgres PVC,
**And** the CRD remains, because Helm never deletes `crds/`, and the Makefile help text says so.

### Story 5.3: Pin the cluster to Kubernetes v1.37.0 by digest

As the builder,
I want the kind node image to match kubectl v1.37.0 and to be pinned by digest,
So that client and server agree on the API, and the node image can't change under the same tag.

*Governed by AD-11. Requires Story 5.1.*

**Acceptance Criteria:**

**Given** `kind/cluster.yaml`,
**When** the pin moves,
**Then** the node image is `kindest/node:v1.37.0@sha256:…`, with the digest taken from the kind v0.33.0 release notes.

**Given** the comment above the pin,
**When** it is rewritten,
**Then** it states the 1.35 cutover (KEP-5573) instead of 1.36, says the host fix from Story 5.1 is applied and needed,
**And** the cut-off sentence about loopback binding is finished.

**Given** a recreated cluster,
**When** `make cluster-down cluster-up load deploy` runs,
**Then** the node reports v1.37.0, and `kubectl version` shows client and server both at v1.37.0,
**And** `make smoke` and `make probe` pass.

**Given** `AGENTS.md`'s pitfall about the node pin,
**When** this story lands,
**Then** that entry is replaced with the new pin and the host requirement it depends on.

---

## Epic 6: Deploy the Operator *(PRD E5b)*

The Operator runs in the cluster with exactly the permissions its markers declare and reports on itself while it does. After this epic the system runs unattended from a single install.

*Phase B · depends on E4, E5 · FR-22, FR-23, FR-27 · AD-8, AD-9, AD-10, AD-11 · NFR4, NFR5, NFR6, NFR8*

**Decisions 2026-10-04, binding on this epic:**
- **Events are emitted on `Ready` transitions** (Story 6.1). FR-22 grants event creation and requires every granted verb to back an identified operation, and until now no story emitted an Event.
- **Metrics are served over plain HTTP with no authn/authz filter** (`--metrics-secure=false`), on a port nothing publishes. kubebuilder's secure default needs `tokenreviews` and `subjectaccessreviews` in the ClusterRole, which would break FR-22's "and nothing else". PRD §7 already excludes TLS and a production posture.

### Story 6.1: Run with exactly the permissions the markers declare

As the cluster's administrator,
I want the Operator to hold exactly the permissions its code uses,
So that the RBAC can be read as a precise list of what the Operator does.

*Realizes FR-22, governed by AD-10 and AD-11.*

**Acceptance Criteria:**

**Given** the reconciler's kubebuilder RBAC markers,
**When** `make manifests` generates RBAC,
**Then** a `ClusterRole` grants `vpntunnels` (get, list, watch, update, patch), `vpntunnels/status` (get, update, patch) and events (create, patch), and nothing else,
**And** the generated RBAC is written into the chart and checked for staleness by `make check`, following the AD-4 pattern.

**Given** the kubebuilder scaffold's defaults,
**When** RBAC is generated,
**Then** the `vpntunnels/finalizers` marker is gone, because nothing here sets `blockOwnerDeletion`,
**And** the leader-election Role is gone, because election is deferred and the Operator runs at `replicas: 1` (AD-11),
**And** the metrics-auth rules for `tokenreviews` and `subjectaccessreviews` are gone, because metrics are served without the auth filter,
**And** no rule touches Services, Endpoints or Secrets, because `gatewayRef` resolves by DNS (AD-10).

**Given** each granted verb,
**When** the markers are read,
**Then** a comment next to each one names the operation that needs it: watch for the informer, update for the finalizer, status update for conditions, event create for transitions.

**Given** a `Ready` condition that changes status or reason,
**When** the reconciler writes it,
**Then** it records an Event on the resource: `Normal` for `Converged`, `Warning` for every failure reason,
**And** a pass that changes nothing records no Event, so a converged resource doesn't produce one every 30 seconds.

**Given** the Operator's identity,
**When** the chart's RBAC is rendered,
**Then** a dedicated ServiceAccount is bound to the ClusterRole by a ClusterRoleBinding, and `default` is never used.

### Story 6.2: Run the Operator in the cluster beside the Gateway

As the gateway's administrator,
I want `make deploy` to install the Operator with the Gateway and Postgres,
So that the system runs unattended from a single install, with no process left running on my host.

*Realizes FR-23, governed by AD-8, AD-10 and AD-11 and by NFR4, NFR5, NFR6 and NFR8.*

**Acceptance Criteria:**

**Given** an Operator Dockerfile,
**When** the image is built,
**Then** it needs no cluster and no `protoc`, both base images are pinned by digest, and the runtime image is distroless and runs as `nonroot`,
**And** `make load` builds and side-loads the Gateway image and the Operator image in the same flow.

**Given** the chart from Story 5.2,
**When** the Operator is added to it,
**Then** it is a Deployment at `replicas: 1` with `imagePullPolicy: IfNotPresent` and leader election off, using the ServiceAccount from Story 6.1,
**And** the image tag and the resync interval are values, with the interval passed to Story 4.1's flag and defaulting to 30s.

**Given** the Operator pod's spec,
**When** it is inspected,
**Then** it carries the same FR-11 posture as the Gateway: explicit non-root uid, all capabilities dropped, no privilege escalation, `RuntimeDefault` seccomp, read-only root filesystem.

**Given** the Operator's liveness and readiness probes, served by controller-runtime,
**When** the Gateway is unreachable,
**Then** liveness keeps passing, carrying NFR8's rule over to the Operator: a pod is never restarted because something it depends on is down.

**Given** the Operator running in the cluster,
**When** it resolves a `gatewayRef`,
**Then** it uses the AD-10 DNS rule, and the dev-only `--gateway-address` flag is not set anywhere in the chart.

**Given** `make deploy` on a clean cluster,
**When** it completes,
**Then** the CRD exists before the Operator pod starts, because `make deploy` applies `crds/` first,
**And** applying a `VpnTunnel` with `gatewayRef: {name: gateway, namespace: netgw}` reaches `Ready=True/Converged` (UJ-1 working inside the cluster).

**Given** the Operator pod's resource requests and limits,
**When** the kind node, Postgres, the Gateway and the Operator run together,
**Then** they fit within the 4 GB ceiling (NFR6), and the measured figure is recorded where the README can quote it.

**Given** a code change to either program,
**When** it is redeployed,
**Then** one Makefile target loads the images and restarts both Deployments, so the fixed `:dev` tag can't leave the old binary running.

### Story 6.3: Expose the controller's metrics

As the operator's author,
I want the standard controller-runtime metrics served from the Operator pod,
So that the loop can be seen working: how often it reconciles, how often it fails, and how much work is queued.

*Realizes FR-27. First in PRD §9's cut order.*

**Acceptance Criteria:**

**Given** the Operator's flags in the chart,
**When** the metrics server starts,
**Then** it serves plain HTTP with `--metrics-secure=false`, so FR-22's ClusterRole needs no metrics-auth rules.

**Given** the Operator pod,
**When** its spec is inspected,
**Then** the metrics port is declared as a named `containerPort`.

**Given** a running Operator that has reconciled at least one resource,
**When** its metrics endpoint is scraped through the API-server proxy (`kubectl get --raw …/pods/<pod>:<port>/proxy/metrics`),
**Then** `controller_runtime_reconcile_total`, `controller_runtime_reconcile_errors_total` and `workqueue_depth` are present for the `vpntunnel` controller.

**Given** a Gateway made unreachable,
**When** several passes have failed,
**Then** the error counter has increased.

**Given** the endpoint's exposure,
**When** the chart is inspected,
**Then** no Service, NodePort or host port publishes it, so it stays inside the cluster.

---

## Epic 7: Legibility *(PRD E6)*

A reviewing engineer with four minutes and no context can tell what was built and why. README with the architecture on the first screen and a 60-second quickstart, ADRs for the three questions a reviewer would ask, and a recording where the restoration beat lands in five seconds.

*Phase C · depends on E6 · realizes UJ-4 · FR-28, FR-29, FR-30 · AD-8, AD-11 · addendum §F is the brief*

**Note:** §9 — never cut. An unbuilt FR costs less than an unreadable repo.

**Decision 2026-10-04:** the demo is recorded with the chart's resync value set to **5s**, stated in a caption alongside the 30s default. Playback runs in real time with no editing, and the five-second beat holds. In UJ-2 the resource never leaves `Ready` and a re-create emits no Event, so the restoration is shown in a pane that polls `ListRoutes`, not in `kubectl get -w`.

### Story 7.1: Publish the decisions as ADRs

As a reviewing engineer,
I want the three decisions I'd question written down with the alternatives that were rejected,
So that I can see the choices were made deliberately.

*Realizes FR-29. Source: the spine's `renderings/ADR-0001..0003`.*

**Acceptance Criteria:**

**Given** the three drafted ADRs (operator rather than CLI, Go operator with Rust gateway, periodic resync),
**When** they are published under `docs/adr/`,
**Then** each one states its rejected alternative in its own section and fits on one printed page.

**Given** lines that were true when drafted and are stale now, such as ADR-0002's "Go must be installed… It currently is not",
**When** they are published,
**Then** every claim matches the repository as built,
**And** anything that points into `_bmad-output/` is rewritten so it stands on its own, because a reviewer reads `docs/adr/` without the planning record.

**Given** `docs/adr/`,
**When** a reader opens it,
**Then** an index lists the ADRs with one line each.

### Story 7.2: Record the drift-correction demo

As a reviewing engineer,
I want to watch a tunnel deleted behind the Operator's back come back by itself,
So that I've seen level-triggered reconciliation work, not just read a claim about it.

*Realizes FR-30 and UJ-2; the evidence for SM-1. The demo beat is five seconds (closed 2026-09-20).*

**Acceptance Criteria:**

**Given** a host shell with the system deployed,
**When** the demo needs to show and break actual state,
**Then** it uses the listing client from Story 1.6 and the owner-carrying delete client from Story 4.1, and adds no new client,
**And** the delete passes the demo resource's `<namespace>/<name>`, without which AD-13 leaves the row in place and nothing happens on screen.

**Given** the recording,
**When** it plays,
**Then** it shows `kubectl apply`, the resource reaching `Ready=True/Converged`, an out-of-band delete, and the route reappearing in a pane that polls `ListRoutes`,
**And** the gap between the delete and the visible restoration is at most five seconds of real-time playback, with the resync chart value set to 5s and a caption giving the 30s default,
**And** because the worst case is one full resync plus one reconcile plus the pane's poll interval, a take that misses five seconds is re-recorded or the resync is set lower, and the caption states the value actually used,
**And** the resource stays `Ready` throughout, so the recording shows that no event drove the repair.

**Given** the recording file,
**When** it is committed,
**Then** it is a format GitHub renders inline, such as an asciinema session converted to GIF, kept to a size that renders quickly,
**And** a script in the repository can re-record it from scratch.

### Story 7.3: Rewrite the README for a four-minute reader

As a reviewing engineer with four minutes and no context,
I want the first screen to tell me what was built and show it working,
So that I can describe the project after closing the tab.

*Realizes FR-28 and UJ-4. Sources: the spine's `renderings/README-architecture-section.md`, addendum §F.*

**Acceptance Criteria:**

**Given** the README's first screen,
**When** it renders on GitHub,
**Then** it shows the mermaid architecture diagram (VpnTunnel → Operator → gRPC → Gateway → Postgres) and the embedded recording from Story 7.2,
**And** the garbled "database's gateway" sentence from the draft is gone.

**Given** the five control-plane ideas,
**When** they are presented,
**Then** each gets its own paragraph that can be read alone, not a table row,
**And** they are weighted as §F.2 says, with level-triggered convergence first and `observedGeneration` presented as the smallest.

**Given** the liveness/readiness split, which §F.2 calls the strongest single item,
**When** it is described,
**Then** the README states the failure it prevents, following §F.1: `tonic_health` reports SERVING unconditionally, `connect()` sends the pod into CrashLoopBackOff, and a restart storm follows.

**Given** the quickstart,
**When** a reader follows it,
**Then** it uses only `make` targets (AD-8), lists its prerequisites (Docker, kind, kubectl, Helm, at stated versions), and reaches a converged `VpnTunnel` in under 60 seconds of commands.

**Given** the NetworkPolicies,
**When** the README mentions security posture,
**Then** it says plainly that kind's CNI does not enforce them, and never describes `netgw` as isolated.

**Given** the README as a whole,
**When** it is checked against the repository,
**Then** it describes no behaviour that isn't built, links the ADRs from Story 7.1, quotes the resource figure measured in Story 6.2, and replaces the current stack-list README.

### Story 7.4: Prove the quickstart on a clean machine

**⚠️ Performed by Lilian. No agent executes any step of this story.**

As the builder,
I want the quickstart run by following the README alone on an environment that has never seen this project,
So that SM-2 is demonstrated rather than assumed.

*Realizes FR-28's verification clause; the evidence for SM-2.*

**Acceptance Criteria:**

**Given** a fresh WSL distro with only Docker and a package manager, and the dev cluster stopped to stay under the 4 GB ceiling,
**When** the README quickstart is followed exactly as written,
**Then** a `VpnTunnel` reaches `Ready=True/Converged`,
**And** the commands typed took under 60 seconds.

**Given** any step the README did not mention,
**When** it is found,
**Then** it is fixed in the README and the run is repeated from scratch,
**And** the story closes only on a run with no undocumented step.

**Given** this story's nature as a host-level action,
**When** it is picked up,
**Then** no agent performs any part of it.
