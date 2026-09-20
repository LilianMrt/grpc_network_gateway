---
stepsCompleted: [1, 2]
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

- **PRD §9 is the authority on epic sequence**, not the PRD's own section numbering. Seven epics: E1, E2, E3, E4, E5a, E5b, E6.
- **Every FR carries a binding `Status:` line.** `Delivered — no story` FRs must not produce stories; they are inventoried here because the Operator's design depends on their exact semantics.
- **Where the PRD and the architecture spine disagree on a structural decision, the spine governs.** Five such overrides are recorded in the spine's *Where This Contradicts Its Inputs* table and are reproduced under Additional Requirements below.
- **No UX design contract exists, and none is expected.** The system's only human surfaces are `kubectl`, the Makefile and the README. The UX Design Requirements section below is deliberately empty.

## Requirements Inventory

### Functional Requirements

Thirty-four FRs. **Ten are `Delivered — no story`** (FR-1, FR-2, FR-4 through FR-11) and generate nothing. **Twenty-four are `To build`** and are the scope of this breakdown, matching PRD §8.1.

> Note: `prds/index.md` still says "32 FRs; 22 to build" — stale as of 2026-09-20, when FR-33 and FR-34 were added by the architecture pass. The PRD body is correct at 34/24.

#### To build — these generate stories

FR-3: The Operator can read every Tunnel the Gateway owns from `vpn_routes` rather than from the serving pod's routing table, via a new `ListRoutes` RPC returning a dedicated three-field `Route` message; an unreachable database returns `UNAVAILABLE`, distinguishable from "no Tunnels exist".
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
- **The AD-11 host cgroup fix has not been applied.** Verified 2026-09-20: no `cgroup_no_v1` in `/proc/cmdline`, 16 v1 controllers mounted, `kind/cluster.yaml` still pinning `v1.33.1` by tag. Requires `kernelCommandLine = cgroup_no_v1=all` in `.wslconfig` and `wsl --shutdown` — a Windows-side file edit and a WSL restart, outside the reach of anything running inside WSL. Owned by E5a per AD-11; unblocks the pin moving to `kindest/node:v1.37.0` by digest.

**Structural work E1 carries beyond its four FRs** (spine, PRD §9 "E1 grew in the architecture pass"):
- Extract every SQL statement into `src/store/`, including the readiness probe's `SELECT 1` (`src/services/health.rs:59`) as `store::ping()`, so persist-before-cache ordering is enforceable rather than remembered (AD-2).
- Replace every `println!` with `tracing`, adding `tracing-subscriber` as a dependency — without it every `tracing` macro emits nothing (AD-9).
- Refresh `AGENTS.md`, three of whose rules are false today (spine: "must be refreshed when E1 lands").

**Architecture invariants binding on stories:**
- **AD-1** — `vpn_routes` is the sole authority; the routing table is a per-pod derived cache. Every control-plane read comes from the store; every write persists before it caches.
- **AD-2** — No SQL reaches Postgres from outside `src/store/`. A handler may call the store then the cache, never the reverse. `network/router.rs` has no database knowledge and gains none.
- **AD-3** — One failure taxonomy mapped at the store boundary into retryable / permanent / conflict; Go classifies only in `operator/internal/gateway`. An error is never rendered as an empty result.
- **AD-4** — `proto/gateway.proto` and `.sqlx/` are generated artefacts that move in the same commit as their source. Go stubs are **committed** to `operator/internal/gatewaypb/`; `go build` and `go test` must never require `protoc`. `make check` runs the generators and fails on `git diff --exit-code`.
- **AD-5** — `operator/internal/controller` must not import `google.golang.org/grpc`. Only `internal/gateway` dials, applies timeouts, or inspects a status code. Tests inject a counting fake.
- **AD-6** — Connections use `grpc.NewClient`, never `grpc.Dial` with `WithBlock` (which would make operator startup depend on gateway availability); cached per `gatewayRef` for the process lifetime and reused.
- **AD-7** — One-way dependency. The operator's only channel to tunnel state is the gRPC API; it never touches Postgres and never imports the generated stubs directly. The gateway never reads the Kubernetes API.
- **AD-8** — The Makefile is the interface, Helm is the mechanism, CRDs are applied outside Helm. `make deploy` runs `kubectl apply -f charts/netgw/crds/` **first**, then `helm upgrade --install`. `k8s/` is deleted when FR-12 lands. Chart is `apiVersion: v2`.
- **AD-9** — Both programs log structurally: Rust `tracing` + `tracing-subscriber`, Go `logr` through controller-runtime. Gateway lines carry the `local_ip` and, on failure, the gRPC code.
- **AD-10** — `gatewayRef` resolves by DNS to `<name>.<namespace>.svc.cluster.local:50051`, never by an API read. **The port is a package constant in `internal/gateway`, not a `gatewayRef` field.** Any namespace is permitted.
- **AD-11** — Operational envelope: one kind cluster `netgw`, single node; both images built locally and side-loaded, `imagePullPolicy: IfNotPresent`, nothing pulled from a registry; every published port binds `127.0.0.1`; Gateway and Operator both `replicas: 1`; the operator watches **cluster-wide**, so a `ClusterRole`. After the host fix, `kindest/node:v1.37.0` by digest; before it, `v1.34.11` is the only kind v0.33.0 prebuilt that boots.
- **AD-12** — `ListRoutes` returns `repeated Route`, a **new** three-field message keyed `local_ip`. The shipped `RouteDetails` is left alone. Bookkeeping columns — `id`, `created_at`, `owner` — are not on the wire. "Already correct" is exact string comparison over `tunnel_id` and `remote_endpoint` **only**.
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
| §6: the cgroup host fix is "deliberately deferred" | AD-11: it is a prerequisite task owned by E5a |
| §6 and `AGENTS.md`: Kubernetes **1.36+** refuses cgroup v1 | Factually wrong — the cutover is **1.35** (KEP-5573); v1.35.8 would also fail |
| `AGENTS.md`: "use `log`, never `tracing`", "no new `println!`" | AD-9 — all three clauses are false today; `tracing` is the dependency |
| `AGENTS.md`: leave the node image pinned at `v1.33.1` | AD-11 — retired once the host fix lands |
| FR-1, FR-2 marked `Delivered — no story` | AD-3 — E1 reopens both handlers to unify the taxonomy |

**Concrete details the reconcile review flags as missing from the PRD, to be carried into acceptance criteria:**
- ~~`status.appliedRoutes`~~ — **CLOSED 2026-09-20: out.** Named in the source notes' reconcile step 7 but never carried into FR-19. The `VpnTunnel` status carries `observedGeneration` and `conditions` only. No story adds this field, and SM-C1 (which penalises added surface area) is the reason.
- The CV bullet is already drafted verbatim and functions as a scope test — every clause maps to an FR: *"Converged declarative network state onto a live gateway by building a Kubernetes operator (Go, controller-runtime) with finalizers and drift-correcting reconciliation over a gRPC control-plane API."*
- **The demo beat is five seconds. CLOSED 2026-09-20.** UJ-2's "five-second proof" and the source notes agree; FR-30 and SM-1's "under ten seconds" is the drift. Acceptance criteria use **five**.
- `examples/smoke_client.rs` already walks create → observe → delete → confirm gone → delete again; it is the skeleton of both the FR-30 demo and the FR-25 integration test. `examples/health_probe.rs` mirrors a Kubernetes `grpc:` probe.

**Addendum §F, the brief for E6** — arguments the README and ADRs must make, not requirements: the liveness/readiness split is the strongest single item and was proved by observation; level-triggered convergence is the most important idea in the controller pattern and the one most often gotten wrong; finalizers are the concrete answer to "why can't you just watch for delete events"; periodic resync signals operational understanding because event-driven alone is the intuitive-but-wrong answer; `observedGeneration` is the smallest of the five but separates current status from stale. Plus the failure each delivered behaviour avoids (§F.1): `tonic_health` defaults `""` to SERVING unconditionally, and `PgPoolOptions::connect()` fails fast into CrashLoopBackOff where a readiness probe cannot help.

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
| FR-12 | **E5a** | The Helm chart for Gateway, Postgres and the CRD; `k8s/` deleted (AD-8) |
| FR-22 | **E5b** | Generated least-privilege RBAC on a dedicated ServiceAccount, cluster-wide (AD-11) |
| FR-23 | **E5b** | The Operator's Deployment added to the chart, with FR-11 hardening |
| FR-27 | **E5b** | The controller-runtime metrics endpoint, port declared on the pod spec |
| FR-28 | **E6** | README: diagram, 60-second quickstart, the five ideas as standalone paragraphs |
| FR-29 | **E6** | Two or three ADRs, each under a page, each naming its rejected alternative |
| FR-30 | **E6** | The UJ-2 recording, restoration visible in five seconds, embedded in the README |

**Not FR-derived, but scoped into epics because the work is real and otherwise unowned:**

| Work item | Epic | Source |
|---|---|---|
| Extract all SQL into `src/store/`, including `store::ping()` | **E1** | AD-2 |
| Replace every `println!` with `tracing`; add `tracing-subscriber` | **E1** | AD-9, Stack table |
| Refresh `AGENTS.md` — three rules are false today | **E1** | Spine, *Where This Contradicts Its Inputs* |
| Install Go 1.27.1 — **performed by Lilian** | **E2** | Spine closing line |
| `kubebuilder` scaffold at `operator/` | **E2** | PRD §8.1, notes M6 |
| WSL cgroup v1 host fix — **performed by Lilian** | **E5a** | AD-11 |
| Move the node image pin to `v1.37.0` by digest | **E5a** | AD-11 |

**`Delivered — no story`:** FR-1, FR-2, FR-4, FR-5, FR-6, FR-7, FR-8, FR-9, FR-10, FR-11. Note that E1 nonetheless reopens the FR-1 and FR-2 handlers, per AD-3 and AD-13 — the FRs stay Delivered; FR-31, FR-33 and FR-34 carry the amendments.

## Epic List

Seven epics, following **PRD §9, which is the authority on sequence**. Each depends only on those before it, and none requires a later epic to function.

### Epic 1: Durable actual-state reads
*Rust · Phase A · depends on nothing*

The Gateway becomes a control-plane API an external reconciler can trust: a caller can read the authoritative set of Tunnels independently of which pod answers, every failure means the same thing whichever RPC produced it, and no resource can destroy a Tunnel it does not own. Delivers standalone value before any operator exists — the known persist-after-cache defect is gone and the API stops lying about why it failed.

**FRs covered:** FR-3, FR-31, FR-33, FR-34
**Also carries:** SQL extraction into `src/store/` (AD-2), `tracing` replacing `println!` (AD-9), the `AGENTS.md` refresh
**Note:** §9 warns this epic has grown past its weekend budget. It is the one place I would look first if the cut order is ever exercised.

### Epic 2: The `VpnTunnel` API
*Go · Phase B · depends on nothing*

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

### Epic 5a: Package the Gateway
*Phase A · depends on nothing*

One command installs the Gateway, Postgres and the CRD into a clean cluster, and one removes them. The Makefile stays the human interface with Helm underneath it, and `k8s/` disappears.

**FRs covered:** FR-12
**Also carries:** the WSL cgroup fix (**performed by Lilian**), then moving the node image pin to `v1.37.0` by digest
**Note:** buildable in Phase A alongside E1 because it packages the Gateway only. AD-8 has made this structural — cutting it now means reverting AD-8, not skipping an epic.

### Epic 5b: Deploy the Operator
*Phase B · depends on E4, E5a*

The Operator runs in the cluster with exactly the permissions its markers declare and reports on itself while it does. After this epic the system runs unattended from a single install.

**FRs covered:** FR-22, FR-23, FR-27

### Epic 6: Legibility
*Phase C · depends on E5b · realizes UJ-4*

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

**Given** the generated-artefact rule (AD-4),
**When** `make check` is run,
**Then** it regenerates `.sqlx/` and fails on `git diff --exit-code`,
**And** the check does not depend on CI, which is deferred.

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

*Realizes FR-34, governed by AD-13.*

**Acceptance Criteria:**

**Given** the schema seeded through the Postgres image's init hooks,
**When** the seed is applied,
**Then** `vpn_routes` carries an `owner` column holding `<namespace>/<name>`,
**And** it is added to the seed, not introduced as a `sqlx migrate` migration.

**Given** `proto/gateway.proto`,
**When** `TunnelRequest` and `DeleteTunnelRequest` gain an `owner` field,
**Then** the regenerated artefacts land in the same commit as the `.proto` edit.

**Given** a row that is unowned or already owned by the caller,
**When** `CreateVpnTunnel` is called,
**Then** the row is claimed or updated.

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

**Given** the validation bounds the gateway enforces — `local_ip` is IPv4, `tunnel_id` and `remote_endpoint` are bounded at their column widths, `remote_endpoint` is `host:port`,
**When** they are settled here,
**Then** they are recorded explicitly so Story 2.2's CRD markers can mirror the same numbers,
**And** the two validators cannot disagree (AD-15).

### Story 1.6: Read actual state from durable storage with `ListRoutes`

As the operator's author,
I want to read every Tunnel the Gateway owns from the durable record,
So that the reconciler's view of actual state does not depend on which pod answered or whether that pod has hydrated.

*Realizes FR-3, governed by AD-1 and AD-12.*

**Acceptance Criteria:**

**Given** `proto/gateway.proto`,
**When** `ListRoutes` is added,
**Then** it returns `repeated Route`, a new message with exactly three fields — `local_ip`, `tunnel_id`, `remote_endpoint` — keyed on `local_ip`,
**And** the shipped `RouteDetails` message is left unmodified, because it is keyed `destination_ip` and belongs to `GetGatewayStatus`.

**Given** the bookkeeping columns `id`, `created_at` and `owner`,
**When** a `Route` is serialized,
**Then** none of them appear on the wire, so a whole-message comparison cannot make an already-correct Tunnel look like drift.

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

### Story 1.7: Correct the three false rules in `AGENTS.md`

As the next agent or human to open this repository,
I want the written rules to match the code,
So that instructions do not actively mislead whoever builds next.

*The spine states plainly: "`AGENTS.md` must be refreshed when E1 lands; three of its rules are wrong today."*

**Acceptance Criteria:**

**Given** the rule "Use the `log` crate — never `tracing`", which is false because `log` is not a dependency and `tracing` is,
**When** `AGENTS.md` is refreshed,
**Then** it states that `tracing` with `tracing-subscriber` is the logging path.

**Given** the rule "no new `println!`", which understated the position,
**When** the file is refreshed,
**Then** it states that no `println!` exists at all and none may be added.

**Given** the SQL boundary established in Story 1.2,
**When** the file is refreshed,
**Then** AD-2 is recorded as a rule: no SQL outside `src/store/`.

**Given** the rule "leave the kind node image pinned at `v1.33.1`",
**When** the file is refreshed,
**Then** the pin stays in place because the host fix has not been applied,
**And** a note records that Story 5a.1 retires it.

---

## Epic 2: The `VpnTunnel` API

A user can declare a Tunnel as a Kubernetes resource and have the API server reject it if it is wrong, before any controller sees it. `kubectl get vpntunnels` renders a useful table. Nothing reconciles yet — and that is a coherent stopping point, because this API is the contract everything downstream is written against.

*Go · Phase B · depends on nothing · FR-13, FR-14 · AD-10, AD-15, conventions: Naming, Identity, Conditions*

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

**Given** the bounds Story 1.5 recorded for the gateway,
**When** the markers are written,
**Then** `tunnelID` and `remoteEndpoint` are bounded at the same column widths the gateway enforces,
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
**And** Story 5a.2 relocates it to `charts/netgw/crds/` when the chart exists.

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
