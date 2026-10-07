<!-- bmad:context -->
<!-- Verified 2026-10-07 against f652686. Managed by bmad-project-context; edits inside this block are replaced on refresh. Keep anything you want preserved outside the markers. -->

## grpc_network_gateway

gRPC control plane for managing virtual VPN tunnels and routing state. Rust (edition 2024),
Tonic + Prost, SQLx against Postgres, deployed to a local kind cluster. Single crate, no
workspace. The Makefile, and the point-of-use comments in `Dockerfile`, `k8s/`, and
`kind/cluster.yaml`, are the design record for what exists. `_bmad-output/planning-artifacts/`
(committed) holds the PRD, the epics, and the architecture spine (`AD-1`..`AD-15`); those are
the authority on *planned* work. `_bmad-output/implementation-artifacts/` holds one spec per
story, `sprint-status.yaml` (the story tracker), `epic-N-context.md`, and `deferred-work.md`.

## Policy

- Work lands on feature branches; never commit or push directly to `main`. Planning artifacts
  under `_bmad-output/` are lightweight: a short-lived `docs/` branch, no review cycle needed. A
  story's own spec, sprint status, and deferred-work entries go in a separate `docs:` commit
  after its code commit, on the story's branch.
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
- `src/store/` owns Postgres, `src/network/router.rs` the in-memory `RoutingTable`,
  `src/services/gateway.rs` the RPC handlers, `src/services/validation.rs` the input bounds,
  `src/services/health.rs` readiness and hydration. Unit tests sit in `#[cfg(test)]` modules in
  `gateway.rs`, `validation.rs`, and `src/store/error.rs`.

## Running and verifying

- Build through the Makefile. Bare `cargo check`/`build`/`run` fail with
  `error communicating with database: Connection refused` when Postgres is down, because
  `sqlx::query!` validates SQL at compile time; `make check`, `make test`, and `make run` set
  `SQLX_OFFLINE=true`.
- Which checks need Postgres: `make check` (type-check, all targets) and `make test` (unit
  tests) run offline, compiling against the committed `.sqlx/` without writing it, so a stale
  `.sqlx/` passes both. `make check-sqlx` needs a running Postgres migrated to the current
  schema: it regenerates `.sqlx/` and fails if `.sqlx/` then differs from git, including when
  regenerated files are correct but not yet committed (it leaves them in the tree). There is no
  CI; these three targets are the automated verification, so run all three before handing work
  over.
- No unit test reaches a working database: every handler test uses `Store::unreachable()`. The
  real SQL — the owner-guarded upsert, the owner-scoped delete, hydration, `ListRoutes` — is
  exercised only by `make smoke` against a running gateway. Run it after changing `src/store/`
  or a handler.
- After changing any SQL in `src/store/`: `make db-up && make migrate && make prepare` (after a
  schema change, `make db-reset && make prepare` instead), then commit the regenerated `.sqlx/`
  in the same commit as the SQL. Without it the offline build and the container image compile
  against stale query data. A fresh compose volume has no tables until `make migrate` runs.
- Keep `sqlx-cli` on the same version as the `sqlx` crate (both 0.9.0); a mismatched CLI
  writes `.sqlx/` data the crate rejects.
- `make smoke`, `make probe`, and `make routes` need a gateway already listening — `make run`,
  or deploy first. `make routes` prints every row through `ListRoutes`; the gateway serves no
  gRPC reflection, so that client is how to inspect actual state.
- Redeploying code to kind: `make load`, then `kubectl rollout restart deployment/gateway -n netgw`.
  `make deploy` re-applies unchanged manifests, and with `imagePullPolicy: IfNotPresent` on the
  fixed `:dev` tag the running pod keeps the old binary.

## Conventions that differ from defaults

- Logging: `tracing`, with the `tracing-subscriber` `fmt` subscriber that `src/logging.rs`
  installs on stdout (filter from `RUST_LOG`, default `info`), is the only logging path.
  `main.rs` calls it right after `dotenv()`. No `println!` remains under `src/` and none may be
  added; do not add the `log` crate. Nothing enforces this, so `grep -rn println src/` must
  stay empty. Put context in fields (`local_ip`, `code`, ...), not in the message. One
  exception: keep the text `hydrated N route(s) from the database` in `src/services/health.rs`,
  which the pod-restart demo quotes. `examples/` keep `println!` because that is CLI output for
  a person.
- No SQL outside `src/store/` (spine AD-2): no `sqlx::query!`, `sqlx::query`, or other path to
  Postgres anywhere else, including health checks (`Store::ping()`). The store never touches the
  `RoutingTable`, and `src/network/` never imports sqlx. Handlers write to the store first and
  update the cache only after it succeeds. Nothing enforces this either:
  `grep -rn sqlx src/ --exclude-dir=store` must stay empty.
- Errors: a store failure becomes a gRPC `Status` in exactly one place, `From<store::Error> for
  Status` in `src/store/error.rs` — `UNAVAILABLE` (database away, retryable), `INVALID_ARGUMENT`
  (bad input or a constraint violation, permanent), `FAILED_PRECONDITION` (`CreateVpnTunnel`
  only: another owner holds the row), `INTERNAL` (unexpected). Handlers reuse it; never classify
  an error by its message text.
- Every route has one `owner`. Callers use `<namespace>/<name>`, but the gateway checks only
  that it is non-empty (an empty one is `INVALID_ARGUMENT`); the column is unbounded `TEXT`.
  Create claims a free row or updates the caller's own. Delete removes only the caller's own;
  deleting an absent or foreign row succeeds with `existed = false`, never an error, so a
  reconciler can retry it.
- Validation bounds in `src/services/validation.rs` equal the `vpn_routes` column widths. Its
  module doc lists the five places they live (the consts, `proto/gateway.proto`, both schema
  copies, and the Story 2.3 CRD markers); change them together.
- `vpn_routes` is the authority on what exists, and `ListRoutes` reads it on every call. Each
  pod's `RoutingTable` is a cache, hydrated once when the database first answers after startup;
  `GetGatewayStatus` and `RoutePacket` serve that pod's cache only and must never stand in for
  `ListRoutes`. Writes update only the serving pod's cache, which is why the Deployment is
  `replicas: 1`.
- The schema exists twice, in `migrations/01_init_routing_table.sql` and
  `k8s/12-configmap-initdb.yaml`; keep them identical. Migration 01 is edited in place rather
  than followed by new migrations, so `make migrate` cannot move an existing database forward:
  after a schema change run `make db-reset` (local) and `make cluster-db-reset` (kind). Both
  destroy the data. `cluster-db-reset` restarts the gateway; after `db-reset`, restart a running
  `make run` yourself, because its cache still holds the old rows.
- Health has two meanings. Liveness (the `liveness` service) is `SERVING` from startup and never
  depends on Postgres, so a database outage cannot restart the pod. Readiness (`""` and the
  gateway's service name) starts `NOT_SERVING` and turns `SERVING` only after the first
  hydration, then follows `Store::ping()`. `src/main.rs` and `k8s/30-gateway.yaml` explain why.

## Known pitfalls

- The NetworkPolicies in `k8s/40-networkpolicy.yaml` are not enforced on kind: kindnet ignores
  them, verified by reaching `postgres:5432` from an unlabelled pod. Never describe the `netgw`
  namespace as isolated on the strength of those manifests.
- The kind node image is pinned at `v1.33.1` because this WSL2 host mounts cgroup v1. The kubelet
  refuses cgroup v1 from Kubernetes **1.35** (KEP-5573 defaults `FailCgroupV1` to true), so
  `v1.35.8` and `v1.36.4` also fail here, and `v1.34.11` is the only kind v0.33.0 prebuilt image
  that boots on this host. Leave the pin alone: Story 5.1 applies the host fix
  (`kernelCommandLine = cgroup_no_v1=all` in `.wslconfig`, then `wsl --shutdown`, spine AD-11)
  and Story 5.3 then moves the pin to `v1.37.0` by digest. Those two stories retire this entry.
- Go is not installed on this host, so nothing under a future `operator/` can be built or tested
  yet. `controller-runtime` needs Go >= 1.26.

<!-- /bmad:context -->
