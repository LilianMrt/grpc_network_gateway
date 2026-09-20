# Addendum — `grpc_network_gateway` control plane

Depth that belongs to downstream documents (architecture, ADRs, README) rather than the PRD. Sourced from `~/context/notes/projects.profile-building.k8s-operator-gateway.md` and from reading the code on 2026-09-19.

## A. Rejected alternatives

### A.1 Go + `kubebuilder` for the Operator, over Rust + `kube-rs`
**Chosen:** Go with `controller-runtime`. **Rejected:** Rust with `kube-rs`, which would keep one language and one toolchain.

Reasons the split wins: `controller-runtime` is the industry standard and is what the Kubernetes repositories of the employers this portfolio targets are written in, so the artifact is instantly legible to its reviewer; it deepens the weaker of the two language claims (Go) rather than reinforcing the stronger (Rust); and the split is *the more realistic architecture*, since a real cloud control plane separates its orchestration layer from the data-plane service it drives. The Operator is a gRPC client of the Gateway, not a module inside it.

The cost is real and should be stated in the ADR: two toolchains, two test harnesses, two build paths in one Makefile.

### A.2 `VpnTunnel` over a `RoutingTable` grouping CRD
An early draft invented a `RoutingTable` kind with `destination`/`nextHop`/`metric` fields. Reading `proto/gateway.proto` and `migrations/01_init_routing_table.sql` killed it: the domain is tunnel-keyed, the unique key is `local_ip`, and there is no notion of metrics or next hops anywhere in the system. A CRD that does not mirror the API it drives produces a reconciler that translates between two invented models. `VpnTunnel` maps one resource to one row to one routing-table entry.

A grouping kind stays interesting as a v2 idea precisely because it makes the diff in FR-16 a genuine set difference rather than a singleton check. It is not worth inventing domain to get there.

### A.3 `ListRoutes` over `GetGatewayStatus` for actual-state reads
`GetGatewayStatus` returns the serving pod's in-memory `HashMap`. Using it as the reconciler's actual-state read makes every reconcile's correctness depend on which pod answered — and on whether that pod had hydrated (PRD §2 Glossary). A new `ListRoutes` reading `vpn_routes` is pod-independent and correct at any replica count.

Also rejected here: making the Gateway re-hydrate periodically so in-memory tables converge across pods. That fixes reads *and* writes, but it duplicates the Operator's drift-correction (PRD §2 Glossary) one layer down. Deferred to Open Question 1 rather than folded into MVP.

Implementation note for architecture: `ListRoutes` must distinguish "database unreachable" from "no rows." Returning an empty list on a failed query would read to the reconciler as total drift and trigger a re-create storm.

### A.4 No PodDisruptionBudget
At one replica, `minAvailable: 1` makes the pod ineligible for voluntary eviction, so `kubectl drain` blocks forever. `maxUnavailable: 1` at one replica is a PDB that permits everything, i.e. no protection. More than one replica is not correct yet (see A.3), so the honest answer is no PDB and a written reason.

## B. Verified evidence behind the delivered FRs

Kept because the PRD asserts these behaviours and the architecture work should not re-derive them.

- **FR-7/FR-8 (health split).** Gateway started with Postgres stopped: probe reported `NotServing`, container stayed `Up`. Postgres started without restarting the container: probe flipped to `Serving`, `RestartCount=0`. Under kind, Postgres scaled to zero: readiness false in ~15s, endpoint removed from the Service, pod `Running` with 0 restarts for 70s against a liveness threshold of 3×10s, recovered unaided.
- **FR-9 (image).** 27.9 MB final image. Both base images pinned by digest. A stub-crate layer builds the dependency tree first so editing source does not recompile vendored openssl. `docker exec /bin/sh` fails; `Config.User = nonroot:nonroot`.
- **FR-10 (cluster + hydration).** Inserted a route, deleted the Gateway pod, replacement logged `hydrated 1 route(s) from the database` and returned Serving. State lives in Postgres; pods are disposable. This is the invariant the reconciler depends on and the closing beat of the demo.
- **FR-11 (NetworkPolicy unenforced).** A pod labelled `app=intruder` reached `postgres:5432` despite the `postgres-ingress` policy. kind's default CNI is kindnet, which does not implement NetworkPolicy; the API server accepts and lists the objects and nothing acts on them.

## C. Host environment gotchas

Not requirements, but they cost a session each and will cost another on a rebuild.

- **cgroup v1 blocks modern Kubernetes.** `kind create cluster` failed at `error execution phase wait-control-plane: context deadline exceeded`, which reads like resource starvation and is not. The kubelet journal *inside the node container* said it directly: `kubelet is configured to not run on a host using cgroup v1`. Kubernetes 1.36+ refuses cgroup v1 and this WSL2 host mounts v1; `systemd=true` in `/etc/wsl.conf` does not change that. Fix applied: pin `kindest/node:v1.33.1`. Permanent alternative, deliberately deferred: `kernelCommandLine = cgroup_no_v1=all` in `.wslconfig`, then `wsl --shutdown`.
- **General lesson:** when a cluster fails to bootstrap, read the kubelet journal inside the node container (`docker exec <node> journalctl -u kubelet`), not the installer's output. The installer only ever reports that the apiserver never answered.
- **Memory.** WSL2 has 3.7 GB total and `rust-analyzer` alone held 1.3 GB. Stop it before creating a cluster.
- **inotify.** `fs.inotify.max_user_instances` was 128 where kind wants 512+. Persisted in `/etc/sysctl.d/99-kind.conf`.
- **Docker bypasses ufw.** Docker's iptables rules skip the host firewall, so a published port binds `0.0.0.0` regardless of firewall rules. Every published port is bound to `127.0.0.1` explicitly, in `compose.yaml` and in the kind port mapping.

## D. Mechanism detail for the architecture pass

- **Write ordering in `DeleteVpnTunnel`.** Postgres delete first, in-memory removal second. A failed database delete therefore leaves a retry seeing an unchanged world. The reverse order would produce a pod whose cache disagrees with the durable record until the next hydration.
- **Upsert in `CreateVpnTunnel`.** `ON CONFLICT (local_ip) DO UPDATE` is what makes FR-16's "update" case require no separate RPC: the reconciler's create call handles both create and change.
- **Wholesale replace in `hydrate()`.** It calls `load_routes` rather than merging, which is why it is safely retryable.
- **The library/binary split.** `src/lib.rs` exists because examples and `tests/` cannot link against a binary target; `main.rs` is a thin binary over it. Required before any integration testing.
- **Schema seeding vs migrations.** Seeded through `docker-entrypoint-initdb.d`; the Gateway runs no migrations and cannot hydrate without the table, so a missing table means readiness never flips. This bypasses `_sqlx_migrations` bookkeeping entirely. If Helm packaging lands, a migration Job is the natural correction.
- **`examples/smoke_client.rs`** already walks create → observe → delete → confirm gone → delete again. It is the skeleton of the FR-30 demo and of the FR-25 integration test.
- **`examples/health_probe.rs`** mirrors what a Kubernetes `grpc:` probe does, and is how the health behaviour above was verified from the host.
- **The kubelet probes the pod IP directly, not the Service.** During the Postgres outage the liveness probe became unreachable *from outside the cluster* precisely because readiness had correctly pulled the pod from the Service — while the kubelet's own liveness probe kept passing throughout. This is the answer to "how do you know the probe was actually still working."


## E. Milestone mapping

Kept so the source notes and the PRD stay reconcilable when the epics are generated. Ordering here follows the source notes' milestone numbering, **not** the build order — PRD §9 is the authority on sequence.

| Notes milestone | PRD | Note |
|---|---|---|
| M0.1 env config | FR-6 | |
| M0.2 DeleteVpnTunnel | FR-2 | |
| M0.3 `cargo sqlx prepare` | FR-9 | |
| M0.4 tonic-health | FR-7, FR-8 | |
| M1 containerise | FR-9 | |
| M2 kind cluster | FR-10 | |
| M3 manifests | FR-10 | |
| M3b loopback binding | §6 Constraints | |
| M4 probes, hardening, netpol | FR-8, FR-11, §7 Non-Goals | |
| M5 Helm | FR-12 | Scoped to the Gateway only; the Operator's chart entry is FR-23 |
| M6 scaffold | FR-13 | Enabling work, no FR of its own |
| M7 types and CRD | FR-13, FR-14 | |
| M8 gRPC client | FR-21 | |
| M9 happy-path reconcile | FR-15, FR-32, FR-16 | |
| M10 finalizers, status, conditions | FR-18, FR-19 | |
| M11 error handling | FR-20 | |
| M12 tests | FR-24, FR-25 | |
| M13 RBAC | FR-22 | |
| M14 observability | FR-26, FR-27 | |
| M15 README | FR-28 | |
| M16 ADRs | FR-29 | |
| M17 demo | FR-30 | |
| CKAD/CKA | §7 Non-Goals | |

**Requirements with no milestone behind them.** Four of these exist only in the PRD; they would have been missed had the epics been generated from the notes alone.

| Source | PRD | Why it has no milestone |
|---|---|---|
| ListRoutes decision, 2026-09-19 | FR-3 | The notes' reconcile step 4 calls an RPC that was never specified |
| Technical-accuracy review, 2026-09-19 | FR-31 | Persist-before-cache on create; the defect was found by reading the code |
| Pre-existing gateway API | FR-1, FR-4, FR-5 | Shipped before the plan; documented because the Operator depends on their semantics |
| Notes' reconcile step 8 | FR-17 | Periodic resync is named in the notes' "five things" but never given a milestone |
| Implied by M5 and M13 | FR-23 | Deploying the Operator itself is assumed everywhere and specified nowhere |

## F. Framing for the README and the ADRs

Source material for FR-28 and FR-29. **Written as directions to the README author**, which is why this section instructs where §§A–E report. These are arguments, not requirements — the PRD deliberately does not rank its own FRs.

### F.1 The failure each delivered behaviour avoids

A requirement states the fix; the argument needs the failure. For FR-7 and FR-8 the naive version is not merely weaker; it is worthless.

- **`tonic_health` starts the overall service (`""`) as SERVING unconditionally.** That is the exact name Kubernetes probes when no service is given. A pod would therefore advertise readiness while Postgres was unreachable, and every request routed to it would fail. A readiness probe wired this way is worse than no probe, because it looks like protection.
- **`PgPoolOptions::connect()` fails fast and kills the process outright when the database is down**, so the port never binds. In Kubernetes that is CrashLoopBackOff — and a readiness probe cannot help a process that is not running. `connect_lazy` is what makes a readiness probe *possible*, not merely accurate.
- **Restarting a process because its database is down fixes nothing.** Every replica fails liveness at the same moment, and the restart storm makes the outage worse while repairing none of it. Readiness alone is the correct response to a failed dependency: it stops traffic without destroying a process perfectly capable of recovering.

### F.2 Which ideas carry the most weight

The five control-plane ideas are not equally hard-won, and the README should not present them as a flat list.

- **The liveness/readiness split is the strongest single item in the project.** It is a real design decision with a real failure mode on the wrong side of it, and it was proved by observation rather than asserted — see §B.
- **Level-triggered convergence is the most important idea in the controller pattern, and the one most often gotten wrong.** The loop converges from whatever state it finds rather than reacting to a change event, which is why it repairs damage no event announced. The README should say it before being asked.
- **Finalizers are the concrete answer to "why can't you just watch for delete events."** Framing them as the answer to that question lands better than describing the mechanism.
- **Periodic resync signals operational understanding** precisely because event-driven alone is the intuitive-but-wrong answer.
- **`observedGeneration` is the smallest of the five**, but it is what separates "status is current" from "status is stale" for any caller.

One rhyme worth stating unprompted: the cache-resync fix rejected in §A.3 is the same drift-correction the Operator performs one layer up — the same idea answering the same class of problem at two levels of the system. It is live in the PRD as Open Questions 1 and 2.
