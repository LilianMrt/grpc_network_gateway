<!-- bmad:context -->
<!-- Verified 2026-09-20 against 4f5fa3f. Managed by bmad-project-context; edits inside this block are replaced on refresh. Keep anything you want preserved outside the markers. -->

## grpc_network_gateway

gRPC control plane for managing virtual VPN tunnels and routing state. Rust (edition 2024),
Tonic + Prost, SQLx against Postgres, deployed to a local kind cluster. Single crate, no
workspace. No planning docs are committed — the Makefile, and the point-of-use comments in
`Dockerfile`, `k8s/`, and `kind/cluster.yaml`, are the committed design record. A local,
gitignored `_bmad-output/planning-artifacts/` additionally holds a PRD and an architecture
spine (`AD-1`..`AD-15`). When present, those are the authority on *planned* work — several
entries below record a decision taken there that the code does not yet reflect.

## Policy

- Work lands on feature branches; never commit or push directly to `main`. Planning artifacts
  under `_bmad-output/` are lightweight: a short-lived `docs/` branch, no review cycle needed.
- Branches reach `main` by rebase and fast-forward only — never a merge commit.
- Never hand-edit `.sqlx/` — it is generated; regenerate with `make prepare`.
- Credentials in `k8s/10-secret.yaml`, `compose.yaml`, and `.env.example` are deliberately
  committed dev values; never put a real secret in a tracked file.

## Where things are

- `make help` lists every target; the Makefile is the entry point for building, running,
  and the kind cluster.
- Protobuf types are generated at build time into `OUT_DIR` and re-exported as
  `services::gateway::proto` — edit `proto/gateway.proto`, never hunt for generated
  files under `src/`.
- `src/main.rs` is a thin binary over `src/lib.rs` so examples and any `tests/` link the
  same modules; new code goes in the library.

## Running and verifying

- Build through the Makefile. Bare `cargo check`/`build`/`run` fail with
  `error communicating with database: Connection refused` when Postgres is down, because
  `sqlx::query!` validates SQL at compile time; `make check` and `make run` set `SQLX_OFFLINE=true`.
- After changing any `sqlx::query!` SQL: `make db-up && make migrate && make prepare`, then
  commit the regenerated `.sqlx/`. Without it the offline build and the container image
  compile against stale query data.
- Keep `sqlx-cli` on the same version as the `sqlx` crate (both 0.9.0); a mismatched CLI
  writes `.sqlx/` data the crate rejects.
- `make smoke` and `make probe` need a gateway already listening — `make run`, or deploy first.
- There is no test suite and no CI; `make check` is the only automated verification.
- Redeploying code to kind: `make load`, then `kubectl rollout restart deployment/gateway -n netgw`.
  `make deploy` re-applies unchanged manifests, and with `imagePullPolicy: IfNotPresent` on the
  fixed `:dev` tag the running pod keeps the old binary.

## Conventions that differ from defaults

- Logging goes through `tracing` (spine AD-9). `src/logging.rs` installs a `tracing-subscriber`
  `fmt` subscriber on stdout (filter from `RUST_LOG`, default `info`), and `main.rs` calls it
  right after `dotenv()`. No `println!` remains under `src/`; do not add one, and do not add the
  `log` crate. Put context in fields (`local_ip`, `code`, ...), not in the message. `examples/`
  keep `println!` because that is CLI output for a person. Story 1.7 completes the refresh of
  this file.
- Tunnel writes update both the in-memory `RoutingTable` and the `vpn_routes` table; reads
  (`GetGatewayStatus`, `RoutePacket`) are served from that pod's memory alone. That is why the
  Deployment is `replicas: 1` — scaling up needs Postgres-backed reads first.

## Known pitfalls

- The NetworkPolicies in `k8s/40-networkpolicy.yaml` are not enforced on kind: kindnet ignores
  them, verified by reaching `postgres:5432` from an unlabelled pod. Never describe the `netgw`
  namespace as isolated on the strength of those manifests.
- The kind node image is pinned at `v1.33.1` because this WSL2 host mounts cgroup v1. The kubelet
  refusal begins at Kubernetes **1.35**, not 1.36 as this file previously said — KEP-5573 defaults
  `FailCgroupV1` to true in 1.35, so `v1.35.8` and `v1.36.4` also fail here, and `v1.34.11` is the
  only kind v0.33.0 prebuilt image that boots on an unfixed host. Spine AD-11 adopts the permanent
  fix (`kernelCommandLine = cgroup_no_v1=all` in `.wslconfig`, then `wsl --shutdown`) and then moves
  the pin to `v1.37.0` by digest. Until that is actually done, leave the pin alone.
- Go is not installed on this host, so nothing under a future `operator/` can be built or tested
  yet. `controller-runtime` needs Go >= 1.26.

<!-- /bmad:context -->
