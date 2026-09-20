---
title: grpc_network_gateway — declarative control plane
status: final
created: 2026-09-19
updated: 2026-09-20
---

# PRD: `grpc_network_gateway` — declarative control plane

## 0. Document Purpose

This PRD is the single planning source for `grpc_network_gateway` and the Kubernetes operator that fronts it. It is written for Lilian as sole builder and for the downstream workflows that consume it (`bmad-create-epics-and-stories`, `bmad-architecture`).

**How to read it.** Vocabulary is anchored in §2 Glossary and used verbatim throughout. Features are grouped in §4 with globally numbered FRs nested under them; FR IDs are stable and do not renumber when features are reorganised, so an FR may appear out of numeric order within a section. **Every FR carries a `Status:` line, and it is binding on downstream workflows:**

- **`Delivered — no story`** — already built and verified. Documented because the Operator's design depends on its exact semantics. **Do not generate a story for it.**
- **`To build`** — outstanding work. Generate a story.

Two inline conventions carry decisions rather than requirements: `[ASSUMPTION: …]` marks an inference not directly confirmed, and `[NOTE FOR PM: …]` marks a deferred decision or an unresolved tension. Both use the bracketed-colon form and both are indexed in §12.

§9 gives the build order and phasing; it, not the section numbering, is the authority on sequence. Upstream input is `~/context/notes/projects.profile-building.k8s-operator-gateway.md`. Rejected alternatives, verification evidence, and mechanism detail live in `addendum.md`.

**Architecture companion.** `../../architecture/architecture-grpc_network_gateway-2026-09-19/ARCHITECTURE-SPINE.md` holds the structural invariants (`AD-1` to `AD-15`). Where this PRD and the spine disagree on a structural decision the spine governs, and its *Where This Contradicts Its Inputs* table records each override. This document was reconciled against it on 2026-09-20. Inline `[ASSUMPTION]` tags are indexed in §12.

## 1. Vision

`grpc_network_gateway` is a Rust gRPC service that owns VPN tunnel routes: it accepts tunnel definitions over gRPC, persists them to Postgres, holds them in an in-memory routing table, and forwards packets against that table. Today it is driven imperatively — a client calls `CreateVpnTunnel`, and whatever the client forgets to call simply never happens.

This project makes that gateway **declarative**. A Kubernetes operator written in Go with `controller-runtime` introduces a `VpnTunnel` custom resource: the cluster holds the desired set of tunnels, the gateway holds the actual set, and a reconciliation loop closes the gap continuously and forever. Deleting a custom resource removes the tunnel through a finalizer instead of orphaning it. A route deleted out of band comes back on the next resync. The operator is a *client* of the gateway's gRPC API, exactly as a real cloud control plane separates orchestration from the data plane it orchestrates.

The reason this shape matters more than the feature list: an IaaS control plane **is** a reconciliation loop, and building one turns "I wrote a Rust gRPC service" into "I built a control plane with a reconciler, finalizers, and drift correction over a gRPC API." That is the sentence the target reader — an infrastructure engineer reviewing the repo — uses about their own work.

### 1.1 Why Now

Kubernetes is the one skill claimed on the CV with nothing behind it: the real experience is Docker plus serverless containers, which is not orchestration. Employers selling managed Kubernetes screen for it directly, so the gap is both a screening filter and an interview hazard. Timing is load-bearing — the artifact is only useful while the job search is live, which is why §9 carries a time budget and a cut order.

## 2. Glossary

- **Gateway** — the Rust gRPC service in this repo (`GatewayController`). Owns tunnel state; exposes the control-plane API. One deployment per namespace.
- **Operator** — the Go `controller-runtime` process in `operator/`. A gRPC *client* of the Gateway. Holds no tunnel state of its own.
- **Tunnel** — one VPN route, uniquely keyed by its **local IP**, carrying a **tunnel ID** and a **remote endpoint**. The Gateway's only domain object.
- **VpnTunnel** — the custom resource, `net.lilianmrt.dev/v1alpha1`, Kind `VpnTunnel`. Exactly one VpnTunnel maps to exactly one Tunnel. Namespace-scoped.
- **local IP** — the unique key of a Tunnel (`vpn_routes.local_ip`, `TunnelRequest.local_ip`, `VpnTunnel.spec.localIP`). Two Tunnels cannot share one.
- **Routing table** — the Gateway's in-memory map of local IP → Tunnel, per process. Rebuilt by **hydration** at startup. Not durable and not shared between pods.
- **`vpn_routes`** — the Postgres table. The durable record of Tunnels; the routing table is a cache of it.
- **Hydration** — the Gateway loading `vpn_routes` wholesale into its routing table. Retryable and idempotent. Runs until it first succeeds, then stops.
- **Desired state** — the set of VpnTunnel resources in the cluster.
- **Actual state** — the durable record in `vpn_routes`, as read through ListRoutes. Deliberately *not* the routing table; see §11 Q2 for what that costs.
- **Drift** — any divergence between desired and actual state, including changes made by actors other than the Operator.
- **Reconcile** — one pass of the Operator's loop over one VpnTunnel: read actual state, diff against desired, apply, write status.
- **Resync** — a Reconcile triggered by elapsed time rather than a cluster event. The mechanism that corrects Drift.
- **Finalizer** — the marker the Operator places on a VpnTunnel so deletion of the resource first removes the Tunnel from the Gateway.
- **`observedGeneration`** — the `metadata.generation` the Operator's `status` was last computed from. Tells a reader whether status reflects the current spec.
- **Readiness** — the Gateway's `""` (overall) gRPC health entry. Follows the database; drives Service membership.
- **Liveness** — the Gateway's `liveness` gRPC health entry. Set SERVING once the process is up and never changed; drives restarts only.
- **`gatewayRef`** — the field on `VpnTunnel.spec` naming which Gateway (name + namespace) owns this Tunnel.

## 3. Target User

### 3.1 Jobs To Be Done

- **As the builder:** convert an unevidenced CV claim into a working artifact he can demo and defend in detail, without faking depth.
- **As the builder:** learn the controller pattern by implementing it against a real external system rather than a toy in-memory store.
- **As the gateway's administrator:** declare the wanted tunnels in one place and stop tracking which imperative calls have and haven't been made.
- **As the reviewing engineer:** decide in four minutes whether this person understands control planes, by reading a README and a demo rather than a claim.

### 3.2 Non-Users (v1)

- **Anyone running this in production.** Credentials are dev-only, the schema is seeded through the Postgres image's init hooks, and the data plane forwards nothing real.
- **Multi-tenant or multi-cluster operators.** One operator, one namespace-scoped gateway, one cluster.
- **Teams.** No RBAC model for human users, no multi-author workflow.

### 3.3 Key User Journeys

- **UJ-1. Lilian declares a tunnel and the cluster makes it true.**
  He writes a `VpnTunnel` manifest — `localIP`, `tunnelID`, `remoteEndpoint`, and a `gatewayRef` pointing at the gateway Service — and runs `kubectl apply`. Within one reconcile the operator has called `CreateVpnTunnel` over gRPC, the row is in `vpn_routes`, and `kubectl get vpntunnels` shows `READY True / Converged` with the tunnel's local IP in a printer column. He never opens a gRPC client. **Edge case:** if the gateway is unreachable, the condition reads `Ready False / GatewayUnreachable` with the gRPC status in the message, and the operator backs off exponentially rather than spinning.

- **UJ-2. A route disappears out of band and comes back on its own.** *(the demo beat)*
  With tunnels converged, Lilian deletes a route directly through the gateway's own API — bypassing Kubernetes entirely, simulating any other actor touching the external system. He watches `kubectl get vpntunnels -w`. Nothing happens for a moment, then the next periodic resync fires. The Operator's `ListRoutes` read shows actual state missing a tunnel the spec still demands, and the Operator re-creates it. The resource never left `Ready`; the world was repaired without an event to react to. This is the five-second proof that the loop is level-triggered rather than edge-triggered.

- **UJ-3. Deleting the resource actually deletes the tunnel.**
  `kubectl delete vpntunnel branch-office-paris`. The resource does not vanish immediately — the finalizer holds it while the operator calls `DeleteVpnTunnel`, which succeeds whether or not the route was still there. Only then is the finalizer removed and the object collected. Lilian confirms via `ListRoutes` that nothing was orphaned. **Edge case:** if the gateway is down, the resource stays in `Terminating` with a condition explaining why, and completes when the gateway returns.

- **UJ-4. A reviewing engineer reads the repo cold.**
  Someone opens the GitHub page with no context and four minutes. The README's first screen shows an architecture diagram — custom resource, operator, gRPC, gateway, Postgres — and an embedded recording of UJ-2. If they keep going, a quickstart brings up a kind cluster and a converged tunnel from a clean machine, and two or three short ADRs explain why an operator rather than a CLI, why Go for the controller and Rust for the gateway, and why periodic resync is not optional. They close the tab able to describe what was built.

## 4. Features

### 4.1 Gateway Control-Plane API

**Description:** The gRPC surface the Operator drives. Every call must be safe to repeat, because a reconcile loop retries freely and cannot reason about partial success. Realizes UJ-1, UJ-2, UJ-3.

#### FR-1: Create a Tunnel
**Status: Delivered — no story.** Shipped 2026-09-14. A client can create a Tunnel by local IP, tunnel ID and remote endpoint.

**Consequences (testable):**
- An invalid `local_ip` returns `INVALID_ARGUMENT` and changes nothing — validation precedes every write.
- Creating a Tunnel whose local IP already exists overwrites its tunnel ID and remote endpoint rather than failing (`ON CONFLICT DO UPDATE`), so re-applying an unchanged spec is a no-op.
- A database failure returns `INTERNAL` and the call reports no success.

**Known defect:** the in-memory write currently precedes the database write, so a failed insert leaves that pod serving a route that was never persisted. Corrected by FR-31.

#### FR-2: Delete a Tunnel, idempotently
**Status: Delivered — no story.** Shipped 2026-09-14. A client can delete a Tunnel by local IP, whether or not it exists. Realizes UJ-3.

**Consequences (testable):**
- Deleting an absent Tunnel returns `success=true, existed=false` — never an error. A finalizer and a retrying Reconcile both depend on this.
- The row is deleted from `vpn_routes` **before** the routing table entry, so a failed database delete leaves a retry seeing an unchanged world.

#### FR-3: List actual state from durable storage
**Status: To build.** The Operator can read every Tunnel the Gateway owns, from `vpn_routes` rather than from the serving pod's routing table.

**Consequences (testable):**
- A new `ListRoutes` RPC returns all rows of `vpn_routes`, independent of which pod serves the call and of whether that pod has hydrated.
- With two Gateway pods and a Tunnel created through pod A, `ListRoutes` served by pod B returns that Tunnel.
- An unreachable database returns `UNAVAILABLE`, distinguishable by the Operator from "no Tunnels exist" — which would otherwise read as total Drift and trigger a storm of re-creates.
- The response is a dedicated three-field `Route` message — `local_ip`, `tunnel_id`, `remote_endpoint` — keyed on local IP, and **not** the shipped `RouteDetails` (which is keyed `destination_ip` and belongs to `GetGatewayStatus`). Bookkeeping columns — `id`, `created_at`, `owner` — are not on the wire, so a whole-message comparison cannot make an already-correct Tunnel look like Drift (AD-12).

**Notes:** `[NOTE FOR PM: this FR is the reason the Operator can trust its actual-state read. GetGatewayStatus (FR-4) cannot serve this purpose and must not be substituted for it.]`

#### FR-31: Persist before caching on create
**Status: To build.** `CreateVpnTunnel` writes the durable record before the in-memory routing table, matching the ordering `DeleteVpnTunnel` already uses.

**Consequences (testable):**
- The Postgres upsert completes successfully before `add_route` is called.
- When the database write fails, the routing table is unchanged and the pod serves no route for that local IP.
- A regression test asserts this: with the database made to fail, a create returns `INTERNAL` and a subsequent `GetGatewayStatus` on that pod shows no such route. *(Folded into FR-25.)*

**Notes:** `[NOTE FOR PM: small change, but it is what makes the "persist first, cache second" argument true of the whole API rather than half of it — and that argument is one worth making out loud.]`

#### FR-33: One failure taxonomy across every RPC
**Status: To build.** The same underlying failure returns the same gRPC status code from every RPC, so the Operator can classify once.

**Consequences (testable):**
- A database that is unreachable returns `UNAVAILABLE` from `CreateVpnTunnel`, `DeleteVpnTunnel` and `ListRoutes` alike. Today the two write paths return `INTERNAL` and FR-3 specifies `UNAVAILABLE`; the same cause must not produce two codes.
- A Postgres constraint violation — a `tunnel_id` longer than the column, a bad type — returns `INVALID_ARGUMENT`, not `INTERNAL`. It is permanent: retrying cannot fix it, and a reconcile loop that treats it as retryable spins forever.
- `INTERNAL` is reserved for genuinely unexpected failures.
- The Operator classifies these in exactly one package and never string-matches a gRPC message.

**Notes:** `[NOTE FOR PM: this amends the observable behaviour of FR-1 and FR-2, which are marked Delivered. It is carried as its own FR rather than by reopening their status, following the precedent FR-31 set. Governed by AD-3 and AD-15.]`

#### FR-34: Every route has one owner
**Status: To build.** A Tunnel row records which `VpnTunnel` owns it, and no resource can destroy a Tunnel it did not create.

**Consequences (testable):**
- `vpn_routes` carries an `owner` column holding `<namespace>/<name>`; `TunnelRequest` and `DeleteTunnelRequest` carry it.
- Creating a Tunnel whose local IP is already owned by a **different** `VpnTunnel` returns `FAILED_PRECONDITION` and changes nothing. Two resources declaring one local IP must not upsert over each other while both report `Ready/Converged`.
- `DeleteVpnTunnel` removes only a row it owns. Deleting a `VpnTunnel` in one namespace leaves another namespace's Tunnel on the same local IP untouched.
- Deleting when the caller owns nothing still returns `success=true, existed=false` — FR-2's idempotency is unchanged.
- The conflict surfaces as a permanent `Ready=False` reason, not a retry.

**Notes:** `[NOTE FOR PM: found by adversarial review of the architecture spine. Without it, §10's SM-5 is violated in the cross-namespace case — a delete silently destroys a converged Tunnel belonging to someone else. Governed by AD-13.]`

#### FR-4: Report the serving pod's routing table
**Status: Delivered — no story.** Shipped before the build plan. The gateway's administrator can inspect what one Gateway pod currently holds in memory, for debugging and for demonstrating hydration.

**Consequences (testable):**
- `GetGatewayStatus` returns the in-memory routing table of the pod that served the call, explicitly not a cluster-wide view.
- After a pod is deleted and rescheduled, the replacement's `GetGatewayStatus` reflects the rows in `vpn_routes` — evidencing that state lives in Postgres and pods are disposable.

#### FR-5: Forward a packet against the routing table
**Status: Delivered — no story.** Shipped before the build plan. The Gateway classifies an IPv4 packet against its routing table.

**Consequences (testable):**
- A packet whose destination matches a Tunnel returns `FORWARDED` with the byte count.
- No match returns `DROPPED (NO_ROUTE)`.
- A malformed or non-IPv4 header returns `DROPPED (MALFORMED_HEADER)`.
- None of the three is an error response; all return OK with an action string.

**Out of Scope:** Actual packet transmission. The data plane is simulated; nothing leaves the process.

### 4.2 Gateway Runtime and Health Semantics

**Description:** How the Gateway behaves as a pod — the behaviours Kubernetes depends on and that most services get wrong.

#### FR-6: Configure from the environment
**Status: Delivered — no story.** Shipped 2026-09-14. The Gateway reads `DATABASE_URL` and `BIND_ADDR` from the environment.

**Consequences (testable):**
- `DATABASE_URL` is mandatory and has no compiled-in default; its absence is a startup error naming `.env.example`.
- `BIND_ADDR` is optional and defaults to `0.0.0.0:50051`, which is the correct default in a container and overridable by config.
- A local `.env` is read when present; its absence is not an error, so the same binary runs in a container unchanged.

#### FR-7: Start without its database
**Status: Delivered — no story.** Shipped 2026-09-14. The Gateway process starts, binds, and serves health regardless of whether Postgres is reachable.

**Consequences (testable):**
- The connection pool connects lazily; an unreachable database at startup does not terminate the process.
- Hydration is retried by a background task until it succeeds, and is safe to run repeatedly.
- Started with Postgres down, the container stays `Up`; started afterwards without touching the container, the Gateway converges to serving with `RestartCount=0`.

#### FR-8: Separate liveness from readiness
**Status: Delivered — no story.** Shipped 2026-09-19. The Gateway exposes two distinct gRPC health entries with different meanings.

**Consequences (testable):**
- Readiness (`""`) reports NOT_SERVING whenever hydration has never succeeded or the database stops answering. With `periodSeconds: 5, failureThreshold: 2` and a 5s health-loop tick, the pod leaves the Service endpoints in **roughly 15 seconds**.
- Liveness (`liveness`) is set SERVING once the process is up — before serving begins — and never depends on the database.
- With Postgres scaled to zero, the pod is marked not-ready within ~15s and stays `Running` with **0 restarts** past the liveness threshold of 3×10s, recovering unaided when Postgres returns.

**Feature-specific NFRs:** Restarting a process because its dependency is down fixes nothing and converts an outage into a simultaneous restart storm across every replica. Liveness must never acquire a database dependency, in this or any future change.

### 4.3 Packaging and Cluster Deployment

**Description:** Everything needed to run the system on Kubernetes from a clean machine.

#### FR-9: Ship a hermetic, minimal container image
**Status: Delivered — no story.** Shipped 2026-09-14. The Gateway builds into a container image requiring no build-time database and offering no runtime shell.

**Consequences (testable):**
- The image builds with Postgres stopped, via `SQLX_OFFLINE=true` against committed `.sqlx/` metadata.
- Both base images are pinned by digest, not tag — a tag can be repointed, which defeats the lockfile.
- The runtime image is distroless: `docker exec /bin/sh` fails.
- The container runs as `nonroot`, uid 65532.
- The final image is under 30 MB.

#### FR-10: Deploy the Gateway and its database to a local cluster
**Status: Delivered — no story.** Shipped 2026-09-15. A kind cluster runs the Gateway, Postgres with a persistent volume, and the configuration they need.

**Consequences (testable):**
- `kind/cluster.yaml` maps the Gateway's NodePort to a host port bound to `127.0.0.1`, so host tooling reaches the cluster with no `port-forward` and nothing is exposed beyond loopback.
- The workflow is covered by named Makefile targets — cluster up/down, image, load, deploy, undeploy, status, logs, smoke, probe — driven as three steps (`cluster-up`, `load`, `deploy`), not one. `[ASSUMPTION: a single aggregate target is a convenience FR-12 will supersede, not a gap worth closing separately.]`
- Postgres runs as a StatefulSet behind a headless Service with a PVC; the Gateway is a Deployment behind a NodePort.
- `imagePullPolicy: IfNotPresent`, because the image exists in no registry and is side-loaded with `kind load docker-image`.
- Deleting the Gateway pod produces a replacement that hydrates the same Tunnels from Postgres and returns to ready.

#### FR-11: Harden the pod
**Status: Delivered — no story.** Shipped 2026-09-19. The Gateway pod runs with least privilege.

**Consequences (testable):**
- `runAsNonRoot` is set with the uid stated explicitly rather than inherited from the image.
- `capabilities: drop: [ALL]` and `allowPrivilegeEscalation: false`.
- `seccompProfile: RuntimeDefault`.
- `readOnlyRootFilesystem: true`, which the binary tolerates.
- NetworkPolicies exist for the namespace, with a header stating plainly that kind's default CNI does not enforce them (§7).

#### FR-12: Package the Gateway and its database as a Helm chart
**Status: To build.** The Gateway, Postgres, and the `VpnTunnel` CRD install into a cluster from one chart. The Operator's own deployment is added by FR-23, so this FR does not depend on the Operator existing.

**Consequences (testable):**
- `helm install` on an empty cluster produces the same running Gateway and Postgres as the raw manifests it replaces.
- Image tag and Gateway replica count are values, not literals.
- The CRD ships in the chart's `crds/` directory, so it installs before any resource that depends on it.
- `helm uninstall` leaves no namespaced objects behind except the PVC.

### 4.4 The `VpnTunnel` API

**Description:** The declarative surface a user actually touches. It mirrors the Gateway's real domain — a Tunnel keyed on local IP — rather than inventing a grouping the Gateway has no concept of. Realizes UJ-1.

#### FR-13: Define the `VpnTunnel` custom resource
**Status: To build.** A user can declare a Tunnel as a Kubernetes resource in group `net.lilianmrt.dev`, version `v1alpha1`.

**Consequences (testable):**
- `spec` carries `gatewayRef` (name + namespace), `localIP`, `tunnelID`, `remoteEndpoint`.
- `localIP` is validated as an IPv4 address by an API-server-enforced marker; an invalid value is rejected at `kubectl apply`, never reaching the Operator.
- `remoteEndpoint` is validated as `host:port`.
- `localIP` is immutable after creation. `[ASSUMPTION: immutability is correct because local IP is the unique key and the Gateway API has no rename operation — changing it is a delete-and-create.]`

**Out of Scope:** Cluster-scoped `VpnTunnel`s; any field with no counterpart in `vpn_routes`.

#### FR-14: Make resource state readable from `kubectl`
**Status: To build.** A user can see whether a Tunnel has converged without describing the resource.

**Consequences (testable):**
- `status` is a subresource, so the Operator's status writes never conflict with a user's spec edits.
- `kubectl get vpntunnels` shows printer columns for local IP, tunnel ID, Ready, and age.
- `status.conditions` uses standard `metav1.Condition` shape via `meta.SetStatusCondition`, with `Ready` reasons including at least `Converged`, `GatewayUnreachable`, and `Deleting`.

### 4.5 Reconciliation

**Description:** The loop itself — the part that makes this a control plane. Every requirement here exists because a loop that converges only from the state it expected is not a reconciler. Realizes UJ-1, UJ-2, UJ-3.

#### FR-15: Run a reconcile loop over `VpnTunnel`
**Status: To build.** The Operator watches `VpnTunnel` resources and reconciles one resource per pass.

**Consequences (testable):**
- Creating, updating, or deleting a VpnTunnel triggers a Reconcile for that resource and no other.
- A Reconcile for a VpnTunnel that no longer exists returns cleanly without error and without a Gateway call.
- Reconcile is scoped to one resource; it never enumerates or mutates sibling resources.

#### FR-32: Read actual state fresh on every pass
**Status: To build.** Reconcile reads actual state from `ListRoutes` (FR-3) at the start of every pass and retains nothing about the Gateway between passes.

**Consequences (testable):**
- No Gateway-derived state is held in a field, package variable, or cache that outlives a single Reconcile call.
- Two consecutive Reconciles each issue their own `ListRoutes` call.
- A Tunnel changed between passes is seen on the next pass without any event having fired.

**Notes:** This is what makes the loop level-triggered rather than edge-triggered — it converges from whatever it finds, not from what it expected to find.

#### FR-16: Diff desired against actual, then apply
**Status: To build.** Reconcile computes what must change before changing anything, and converges from any starting state.

**Consequences (testable):**
- Given the Tunnel absent, present-but-wrong, or already correct, one Reconcile leaves it correct in all three cases.
- A Tunnel in spec and absent from actual state produces a `CreateVpnTunnel` call.
- A Tunnel present with a differing tunnel ID or remote endpoint produces a single create call, which upserts (FR-1).
- A Tunnel already matching produces **no** gRPC write call — verified by call count, not by outcome.

#### FR-17: Correct drift on a timer
**Status: To build.** Reconcile runs periodically even when nothing in the cluster changed. Realizes UJ-2.

**Consequences (testable):**
- Every Reconcile returns a requeue-after interval, so each VpnTunnel is re-examined on a bounded schedule.
- The interval defaults to **30 seconds** and is configurable through the Helm chart (FR-12). Chosen so the demo beat (SM-1) is watchable in real time without editing, while staying far below any load the Gateway would notice at this scale.
- A Tunnel deleted directly through the Gateway API is restored **within 30 seconds**, with no cluster event involved.

#### FR-18: Remove Tunnels through a finalizer
**Status: To build.** Deleting a VpnTunnel removes its Tunnel from the Gateway before the resource is collected. Realizes UJ-3.

**Consequences (testable):**
- The finalizer is added **before** the first call that creates external state, so a crash between the two cannot orphan a Tunnel.
- With `deletionTimestamp` set, Reconcile calls `DeleteVpnTunnel`, removes the finalizer, and returns — performing no create.
- Deletion succeeds even when the Tunnel is already gone, relying on FR-2's idempotency.
- While the Gateway is unreachable, the resource remains in `Terminating` with an explanatory condition, and completes once the Gateway returns.

#### FR-19: Publish honest status
**Status: To build.** A reader can tell whether status reflects the current spec.

**Consequences (testable):**
- `status.observedGeneration` is set to `metadata.generation` only after a successful converge.
- Editing the spec leaves `observedGeneration` behind the generation until the next successful Reconcile.
- The `Ready` condition's `reason` distinguishes converged from every failure mode the Operator can observe.

#### FR-20: Fail by returning, never by sleeping
**Status: To build.** Errors are handled by the controller runtime, not inside the loop.

**Consequences (testable):**
- Reconcile returns the error and lets `controller-runtime` apply exponential backoff; no `time.Sleep` appears in the reconcile path.
- A persistently unreachable Gateway produces a backing-off retry pattern in logs, not a tight loop and not a wedged worker.

### 4.6 Operator Deployment and Access

**Description:** Running the Operator in the same cluster as the Gateway, with the access it needs and nothing more. Realizes UJ-1.

#### FR-21: Reach the Gateway over gRPC
**Status: To build.** The Operator connects to the Gateway named by each resource's `gatewayRef`.

**Consequences (testable):**
- The target address is resolved from `gatewayRef` (name + namespace) to the Gateway's in-cluster Service DNS.
- Every call carries a timeout; no call can block a worker indefinitely.
- A connection failure surfaces as a typed error that FR-19 renders into the `Ready` condition.

#### FR-22: Run with least-privilege RBAC
**Status: To build.** The Operator has exactly the permissions its markers declare.

**Consequences (testable):**
- Role and binding are generated from kubebuilder RBAC markers, not hand-written.
- The Operator uses a dedicated ServiceAccount, never `default`.
- Permissions cover `vpntunnels` and `vpntunnels/status` plus event emission, and nothing else; removing any granted verb breaks a specific, identified operation.

#### FR-23: Deploy the Operator alongside the Gateway
**Status: To build.** The Operator's Deployment, ServiceAccount, and RBAC are added to the chart FR-12 established.

**Consequences (testable):**
- The Operator image is built and side-loaded by the same Makefile flow as the Gateway image.
- The Operator pod carries the same hardening posture as FR-11.
- The resync interval (FR-17) is a chart value.
- After `helm install`, the CRD is present before the Operator pod starts.

### 4.7 Verification

**Description:** Evidence the loop behaves as claimed, against a real API server and a real Gateway. Realizes UJ-2, UJ-3.

#### FR-24: Test the controller against a real API server
**Status: To build.** Controller behaviour is verified with `envtest`.

**Consequences (testable):**
- Tests cover, at minimum: converge-from-absent, converge-from-wrong, no-op on already-correct, finalizer removal on delete, and `observedGeneration` lagging a spec edit until reconciled.
- An idempotency test asserts that running Reconcile repeatedly against a converged resource produces no further Gateway write calls.
- Tests run without a cluster, on a clean machine.

#### FR-25: Test against a real Gateway
**Status: To build.** Integration tests run the Gateway and Postgres as containers via `testcontainers-go`.

**Consequences (testable):**
- At least one test drives a full create → `ListRoutes` → delete → re-delete cycle through the real gRPC API.
- The drift case from UJ-2 is covered: delete a Tunnel behind the Operator's back, run Reconcile, assert restoration.
- The FR-31 regression is covered: with the database failing, a create leaves no in-memory route.

### 4.8 Observability

**Description:** What the Operator reports about itself while it runs — the difference between a loop that works and a loop you can watch working. Realizes UJ-4.

#### FR-26: Log structurally
**Status: To build.** The Operator logs through `logr` with the resource under reconciliation in context.

**Consequences (testable):**
- Every log line from a reconcile carries the resource's namespace and name.
- Gateway call failures log the gRPC status code, not only a message string.

#### FR-27: Expose controller metrics
**Status: To build.** The standard `controller-runtime` metrics endpoint is served and reachable.

**Consequences (testable):**
- Reconcile count, error count, and queue depth are scrapeable from the Operator pod.
- The metrics port is declared on the pod spec.

### 4.9 Repository Legibility

**Description:** The work that makes the rest of it land with a reader who has four minutes. Realizes UJ-4. An unreadable repo wastes the whole build.

#### FR-28: README that works from a clean machine
**Status: To build.** A reader can understand the architecture from the first screen and run the system from the quickstart.

**Consequences (testable):**
- The first screen carries an architecture diagram showing VpnTunnel → Operator → gRPC → Gateway → Postgres.
- The quickstart reaches a converged Tunnel on a machine with only Docker and a package manager, in **under 60 seconds of commands**, and is verified by actually running it on a clean environment.
- The five control-plane ideas — level-triggered convergence, idempotency, finalizers, `observedGeneration`, periodic resync — are each stated in a paragraph a reviewer can read standalone.

#### FR-29: Record the decisions as ADRs
**Status: To build.** Two or three short ADRs explain the choices a reviewer would otherwise question.

**Consequences (testable):**
- At minimum: why an operator rather than a CLI; why Go for the Operator and Rust for the Gateway; why periodic resync is required rather than optional.
- Each is under a page and states the rejected alternative.

#### FR-30: Record the demo
**Status: To build.** A recording shows UJ-2 end to end.

**Consequences (testable):**
- The recording shows `kubectl apply`, the resource going Ready, an out-of-band delete, and automatic restoration — the restoration beat visible in under ten seconds of playback.
- It is embedded in the README, not only linked.

## 5. Cross-Cutting NFRs

- **Idempotency is a system property, not a feature.** Every Gateway write and every Reconcile must be safe to repeat. Any new RPC added later inherits this.
- **No external state cached across reconciles** (FR-32). This is what makes the loop level-triggered.
- **The loop never blocks.** No sleeping inside Reconcile; all waiting is expressed as requeue or as call timeouts (FR-20, FR-21).
- **Least privilege throughout.** Non-root, all capabilities dropped, read-only root filesystem, seccomp default, generated RBAC. Applies to both the Gateway and the Operator pods.
- **Builds are hermetic.** Neither image requires a reachable database or a live cluster to build. Base images pinned by digest.
- **Resource ceiling.** The whole system — kind node, Postgres, Gateway, Operator, and the test harness — must run on a host with under 4 GB of usable RAM. Anything that does not fit is deferred or dropped, and §7 says which. `[ASSUMPTION: the 3.7 GB WSL2 host remains the development environment.]`
- **Failure is legible.** Every failure mode a user can hit surfaces in `kubectl get`/`describe` output with a distinguishing reason, not only in Operator logs.

## 6. Constraints and Guardrails

- **Dev credentials only.** Secrets in the manifests are development values matching `compose.yaml` so local and cluster behave identically. Not a production posture, and stated as such in the manifests.
- **Schema is seeded, not migrated.** The Gateway does not run its own migrations and cannot hydrate without the table, so the schema is applied through the Postgres image's init hooks. This bypasses sqlx's migration bookkeeping and is development-only. `[NOTE FOR PM: FR-12 makes a migration Job cheap to add — reconsider there.]`
- **Kubernetes version is pinned, and the host fix is no longer deferred.** The local cluster pins a node image at v1.33.1 today because this WSL2 host mounts cgroup v1. Two corrections to what this section previously said: the cutover is **Kubernetes 1.35**, not 1.36 — KEP-5573 defaults `FailCgroupV1` to true in 1.35, so v1.35.8 and v1.36.4 fail here too, and v1.34.11 is the only `kind` v0.33.0 prebuilt image that boots on an unfixed host. And the permanent fix (`kernelCommandLine = cgroup_no_v1=all` in `.wslconfig`, then `wsl --shutdown`) is **adopted, not deferred**: AD-11 makes it a prerequisite owned by E5a, after which the pin moves to v1.37.0 by digest and matches kubectl. As of 2026-09-20 it has not been applied and `kind/cluster.yaml` still pins v1.33.1.
- **No registry.** Images exist only locally and are side-loaded into kind; any manifest that triggers a pull fails.
- **Published ports bind to loopback.** Docker's iptables rules bypass the host firewall, so every published port is explicitly bound to `127.0.0.1`.

## 7. Non-Goals (Explicit)

*Never, at any version. Contrast §8.2, which defers rather than excludes.*

- **Enforced NetworkPolicy.** The policies are written and correct, but kind's default CNI does not implement NetworkPolicy — verified directly: a pod reached Postgres despite an ingress policy denying it. Enforcing them means recreating the cluster without its default CNI and running Calico, which does not fit the host's memory ceiling. The manifests stay, with their header stating plainly that they are unenforced. *An unenforced NetworkPolicy presented as protection is worse than none.*
- **Kubernetes certification (CKAD/CKA).** A credential, tracked separately. Never a substitute for the artifact.
- **A real data plane.** No packets leave the process; `RoutePacket` classifies and reports.
- **Production readiness.** No TLS between Operator and Gateway, no managed secrets, no HA Postgres, no backup or restore.
- **A PodDisruptionBudget.** At one replica, `minAvailable: 1` makes the pod ineligible for voluntary eviction and a node drain blocks forever. Revisit only if the Gateway becomes genuinely multi-replica.
- **Rust-based operator (`kube-rs`).** Considered and rejected; rationale in `addendum.md` §A.1.
- **Multi-tenancy, multi-cluster, or cross-namespace Gateway references beyond `gatewayRef`'s explicit namespace.**

## 8. MVP Scope

### 8.1 In Scope

The work to build is **FR-3, FR-12 through FR-32, and FR-33 to FR-34** — twenty-four requirements, sequenced in §9. FR-33 and FR-34 were added on 2026-09-20 from the architecture pass; both amend behaviour of Delivered FRs and are carried as their own FRs rather than by reopening a `Delivered — no story` status. FR-1, FR-2, and FR-4 through FR-11 are delivered and carry no stories; they are in this document because the Operator's design depends on their exact semantics.

The Operator lives in this repository under `operator/`. The bilingual tree is deliberate: it shows the orchestration layer and the data-plane service as separate programs in separate languages, which is the real architecture, and it keeps one link for a reader. `[ASSUMPTION: the Operator is scaffolded with kubebuilder at operator/, with a Go module independent of the Rust crate.]`

### 8.2 Out of Scope for MVP

*Deferred past MVP and open to revisit. Contrast §7, which excludes permanently.*

- **Multi-replica Gateway.** `ListRoutes` (FR-3) makes the Operator's *reads* correct at any replica count, but writes still mutate only the serving pod's routing table, so a Tunnel created through pod A is invisible to pod B's `RoutePacket` for that pod's lifetime. MVP runs one replica. **Deferred, not closed** — see §11 Q1. `[ASSUMPTION: replicas=1 for the Gateway throughout MVP.]`
- **Webhooks** — validating or defaulting admission. CRD markers (FR-13) cover the validation this API needs; a webhook adds certificate management for no gain here.
- **Leader election for the Operator.** One replica, no contention. Worth adding the moment a second replica is contemplated.
- **A grouping CRD** that owns many Tunnels in one resource. Named v2 idea; `v1alpha1` ships the singleton that matches the Gateway's domain.
- **CI.** Tests are runnable locally and in CI, but wiring a pipeline is not in MVP. `[NOTE FOR PM: cheap once FR-24/FR-25 exist, and it strengthens UJ-4 materially. Revisit if the timeline allows.]`
- **Updating the CV.** Gated on the code being pushed and the README being readable — not on the code merely working.

## 9. Build Order and Phasing

**This section is the authority on sequence.** Section numbering is not. Six epics; each depends only on those before it.

| # | Epic | FRs | Depends on | Phase |
|---|---|---|---|---|
| E1 | Durable actual-state reads *(Rust)* | FR-3, FR-31, FR-33, FR-34 | — | A |
| E2 | The `VpnTunnel` API *(Go)* | FR-13, FR-14 | — | B |
| E3 | Converge a tunnel *(UJ-1)* | FR-21, FR-15, FR-32, FR-16, FR-19, FR-20, FR-26 | E1, E2 | B |
| E4 | Drift and deletion *(UJ-2, UJ-3)* | FR-17, FR-18, FR-24, FR-25 | E3 | B |
| E5a | Package the Gateway | FR-12 | — | A |
| E5b | Deploy the Operator | FR-22, FR-23, FR-27 | E4, E5a | B |
| E6 | Legibility *(UJ-4)* | FR-28, FR-29, FR-30 | E5b | C |

**Dependency notes that are not obvious from the table.** E1 must land before E3 because FR-32's fresh read has nothing to call without FR-3. E2 must land before E3 because there is no resource to reconcile without the CRD. E5a packages the Gateway only and depends on nothing, which is why it can be done in Phase A alongside E1; E5b then adds the Operator to that chart. The two are sequential, not mutually dependent. E6 comes last because a README describing unbuilt behaviour is worse than no README.

**E1 grew in the architecture pass.** Beyond its four FRs it also carries structural work that no FR describes: extracting every SQL statement into `src/store/` so the persist-before-cache ordering is enforceable rather than remembered (AD-2), and replacing `println!` with `tracing` (AD-9). Both are one pass through the same files. This is a deliberate trade against SM-C3, taken because every item is something the Operator's design already assumes.

**Time budget.** Phase A (E1 and E5a) is a weekend — **E1 as now scoped will likely exceed that**; Phase B (E2–E4, the operator itself) is two to three weeks and is the differentiator — it is the part worth not rushing. Phase C (E6) is a few days and must not be skipped: an unreadable repo wastes the whole build.

**Cut order if time runs short.** Drop FR-27 (metrics) first, then FR-12 (fall back to raw manifests), then FR-24 (keep FR-25, which covers the demo path). **Never cut E6** — an unbuilt FR costs less than an unreadable repo. `[NOTE FOR PM: if the cut order is ever exercised, revisit SM-2 (which rests on FR-12) and SM-5 (which rests on FR-24).]`

## 10. Success Metrics

**Primary**
- **SM-1: The demo beat lands.** An out-of-band Tunnel deletion is corrected automatically within the 30s resync interval, visible in a recording without editing. Validates FR-17, FR-25, FR-30.
- **SM-2: Cold-start quickstart works.** A clean machine reaches a converged `VpnTunnel` by following the README only, in under 60 seconds of commands, with no undocumented step. Validates FR-10, FR-12, FR-28.
- **SM-3: The five ideas are defensible unprompted.** Level-triggered convergence, idempotency, finalizers, `observedGeneration`, and periodic resync can each be explained in conversation, from this implementation, without notes. Validates FR-16, FR-32, FR-18, FR-19, FR-17, FR-29.

**Secondary**
- **SM-4: The CV line changes.** `Kubernetes (fundamentals)` becomes a substantiated skill token, backed by a repo bullet in the shape of *"Converged declarative network state onto a live gateway by building a Kubernetes operator (Go, controller-runtime) with finalizers and drift-correcting reconciliation over a gRPC control-plane API."* Gated on push + readable README.
- **SM-5: Nothing orphans.** Across the full test suite, no run leaves a Tunnel in `vpn_routes` without a corresponding VpnTunnel. Validates FR-18, FR-24.

**Counter-metrics (do not optimize)**
- **SM-C1: FR count and feature breadth.** Adding surface area to the CRD or the Gateway API makes the artifact worse, not better. Depth on the five ideas is the entire value. Counterbalances SM-3.
- **SM-C2: Test coverage percentage.** The named behaviours in FR-24 and FR-25 matter; the number does not. Counterbalances SM-5.
- **SM-C3: Time before first push.** A polished-but-unpushed repo scores zero against every metric here. Counterbalances SM-2.

## 11. Open Questions

**Q3, Q4 and Q5 were closed by the architecture pass on 2026-09-20** and are kept below with their answers, since the reasoning is part of the record. Q1 and Q2 remain genuinely open and are carried under *Deferred* in the spine.

1. **Does the Gateway ever become genuinely multi-replica, and if so how do writes propagate?** `ListRoutes` fixes control-plane reads only. The candidate fix is periodic re-hydration inside the Gateway so pods converge on `vpn_routes` — which is the same drift-correction the Operator performs, one layer down. The idea appearing twice in this project is worth saying out loud. *Blocks nothing in MVP; decide before any change to `replicas`.*
2. **Should the Operator be able to observe cache-vs-durable divergence at all?** §2 defines actual state as the durable record, so a pod whose routing table disagrees with Postgres is invisible to the reconciler, and `Ready/Converged` can coexist with stale forwarding. Hydration only runs until it first succeeds, so nothing re-syncs that cache within a pod's lifetime. Acceptable at one replica; a real gap the moment Q1 is answered. `[ASSUMPTION: cache-vs-durable divergence is acceptable at one replica and is not a condition the Operator must surface.]` *Related to FR-4, which can see exactly what FR-3 cannot.*
3. **CLOSED — `ListRoutes` reads `vpn_routes` directly.** It is a pure store read with no cache interaction (AD-1, AD-12), so a read RPC never mutates state and the Operator's resync cadence never silently becomes the Gateway's cache-freshness policy. Re-hydration was rejected: at `replicas: 1` it buys very little, because UJ-2's out-of-band delete goes through the Gateway API and clears both row and cache, and the Operator's re-create rewrites both. Cache convergence remains the Gateway's own concern, deferred to Q1.
4. **CLOSED — yes, any namespace (AD-10).** The reason previously given here does not hold: resolving `gatewayRef` to `<name>.<namespace>.svc.cluster.local:50051` is a name construction plus a gRPC dial, with no Kubernetes API read, so it carries no extra RBAC verb. The real cost is consent — a `VpnTunnel` in one namespace can drive a Gateway in another without the target agreeing — and §7 already declares multi-tenancy a non-goal. FR-34's row ownership removes the destructive half: nobody can delete a route they do not own. The Operator watches cluster-wide with a `ClusterRole` (AD-11).
5. **CLOSED — the Makefile is the interface, Helm is the mechanism (AD-8).** `make deploy` applies the CRD with `kubectl` and then runs `helm upgrade --install`; the README quickstart is `make` targets only; `k8s/` becomes the chart's templates and the directory is removed by FR-12. One deployment path, and a reader who opens the Makefile sees Helm immediately. Note that `helm upgrade` does **not** update resources in `crds/`, which is why the CRD is applied outside the chart.

## 12. Assumptions and Notes Index

Every `[ASSUMPTION]` tag in this document, for explicit confirmation:

- §11 Q2 — cache-vs-durable divergence is acceptable at one replica and is not a condition the Operator must surface.
- §4.3 FR-10 — a single aggregate Makefile target is a convenience FR-12 supersedes, not a gap worth closing separately.
- §4.4 FR-13 — `localIP` immutability is correct because it is the unique key and the Gateway API has no rename.
- §5 — the 3.7 GB WSL2 host remains the development environment; the resource ceiling derives from it.
- §8.1 — the Operator is scaffolded with `kubebuilder` at `operator/`, with a Go module independent of the Rust crate.
- §8.2 — the Gateway runs at `replicas=1` throughout MVP.

Every `[NOTE FOR PM]` callout, for revisit:

- §4.1 FR-3 — `GetGatewayStatus` must never be substituted for `ListRoutes` as the actual-state read.
- §4.1 FR-31 — the persist-first argument is only true of the whole API once this lands.
- §4.1 FR-33 — the taxonomy amends Delivered FR-1/FR-2 behaviour; carried as its own FR rather than by reopening their status.
- §4.1 FR-34 — without row ownership, SM-5 is violated in the cross-namespace case: a delete silently destroys another resource's converged Tunnel.
- §6 Constraints — FR-12 makes a migration Job cheap to add; reconsider seeding-vs-migrations there.
- §8.2 — CI is cheap once FR-24/FR-25 exist and strengthens UJ-4; revisit if the timeline allows.
- §9 — if the cut order is exercised, revisit SM-2 and SM-5.
