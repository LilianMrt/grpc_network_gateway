---
title: 'Move every SQL statement behind src/store/'
type: 'refactor'
created: '2026-10-04'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'b143bfd264c2b97328f88aac990a9c845a1fc52c'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** SQL is spread across `gateway.rs` (three `query!` calls), `health.rs` (`SELECT 1`), and `main.rs` (pool construction). Write ordering against the in-memory cache is therefore remembered, not enforced, and create gets it wrong (cache before database).

**Approach:** Add a `src/store/` module that owns the pool and every statement, behind a `Store` handle. Handlers and the readiness task call it, store first and cache second. Regenerate `.sqlx/`, and add `make check-sqlx` so stale offline data fails loudly (spine AD-3, AD-4).

## Boundaries & Constraints

**Always:**
- Outside `src/store/`, nothing names `sqlx`: no `query!`, no `sqlx::query`, no `PgPool`, no `sqlx::Error`. The store exposes `pub type Error = sqlx::Error;` as the seam Story 1.5 replaces with its taxonomy.
- `Store` owns pool construction. It keeps `connect_lazy` and `max_connections(5)`, and still never touches Postgres at startup. Story 1.5 sets the acquire timeout here.
- The store never imports `crate::network`, and `router.rs` never imports the store or sqlx.
- Every caller that touches both runs store first, cache second: create, delete, and `hydrate`. Moving create's `add_route` after the upsert is required by this story's AC. Story 1.3 still owns proving the failure behaviour.
- gRPC responses, status codes and status messages stay byte-identical, and so do the 1.1 log lines and fields.
- `make check` stays offline and never writes `.sqlx/`.
- Decision (2026-10-04, user): readiness failures get a cause. When `store.ping()` or `hydrate` fails, a `warn!` carries `error = %err`. It is logged only on the transition to not-ready, so an outage does not log every 5s and `RUST_LOG=warn` still shows it. This closes the readiness entry in `deferred-work.md`.
- Decision (2026-10-04, user): keep the full spec at about 1,800 tokens; no split.

**Never:**
- No change to the schema, the proto, the error taxonomy (1.5), ownership (1.4), or `ListRoutes` (1.6).
- No pool acquire timeout yet, and no AGENTS.md edit (Story 1.7).
- Never hand-edit `.sqlx/`.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Create, DB up | valid request | row upserted, then cached; same success response | N/A |
| Create, DB down | valid request | `Internal`, same message as today | no `add_route` runs; `GetGatewayStatus` shows no route for that IP |
| Delete | existing or absent row | row removed, then cache entry removed; `existed` as today | DB error → `Internal`, cache untouched |
| Readiness, DB down | after hydration | `store.ping()` fails → NOT_SERVING | one `WARN` with `error` on the transition, then quiet |

</frozen-after-approval>

## Code Map

- `src/main.rs:13,41,45,59` -- `PgPoolOptions` import and `connect_lazy`. Replace them with `Store::connect_lazy(&database_url)?` and pass `store` to `Gateway::new` and `spawn_readiness_task`.
- `src/services/gateway.rs:20,26-54` -- `hydrate(pool, table)`. Its `query!` SELECT moves to the store; `hydrate` stays here (it fills the cache) and takes `&Store`. Keep its `warn!` for an unparseable `local_ip`.
- `src/services/gateway.rs:56-66` -- `Gateway { routing_table, db_pool: PgPool }` becomes `store: Store`.
- `src/services/gateway.rs:101-123` -- create: `add_route` at 101 runs before the INSERT at 103. Reorder. The `map_err` that builds `Status::internal("Database persistence failure: {err}")` and logs it stays in the handler, unchanged.
- `src/services/gateway.rs:154-169` -- delete: already store first. The store returns `existed: bool` (`rows_affected() > 0`).
- `src/services/health.rs:18,49,60,62` -- `PgPool` parameter, `sqlx::query("SELECT 1")`, `hydrate(&pool, …)`.
- `src/lib.rs` -- register `pub mod store;`.
- `.sqlx/` -- three query files. Query text that changes produces new hashes; `make prepare` (`--all-targets`) rewrites the directory.
- `Makefile:25-31` -- `prepare` and `check`. Add `check-sqlx` next to them.
- `Dockerfile` builds with `SQLX_OFFLINE=true` from `.sqlx/`; `COPY . .` picks up `src/store/` unchanged.
- `compose.yaml` and `migrations/` -- the local Postgres that `make prepare` and `make check-sqlx` need (`make db-up && make migrate`).

## Tasks & Acceptance

**Execution:**
- [x] `src/store/mod.rs` (new), `src/lib.rs` -- `#[derive(Clone, Debug)] pub struct Store { pool }`, with `connect_lazy(url)`, `ping()`, `list_routes() -> Vec<StoredRoute>` (`local_ip`, `tunnel_id`, `remote_endpoint`, all `String`), `upsert_route(local_ip, tunnel_id, remote_endpoint)`, `delete_route(local_ip) -> bool`, and `pub type Error`. A module doc states the one-way rule. Clean up the INSERT's trailing whitespace while moving it.
- [x] `src/services/gateway.rs` -- use `Store`; reorder create to upsert, then `add_route`; delete and hydrate call the store.
- [x] `src/services/health.rs` -- take `Store`, call `store.ping()`; no sqlx import. Keep the failure's error, and on the transition to not-ready log `warn!(%status, error = %err, ...)` in place of the `info` line. The transition back to serving stays `info`.
- [x] `src/main.rs` -- build the `Store`; drop the sqlx import and keep the lazy-connect comment.
- [x] `.sqlx/` -- `make db-up && make migrate && make prepare`; commit the result in the same commit as the moved SQL.
- [x] `Makefile` -- `check-sqlx` target (`## Regenerate .sqlx and fail if it differs from git (needs Postgres)`): run `cargo sqlx prepare -- --all-targets`, then fail when `git status --porcelain -- .sqlx` is non-empty. `git diff --exit-code` alone misses new untracked query files.

**Acceptance Criteria:**
- Given the tree, when `grep -rn sqlx src/ --exclude-dir=store` runs, then it prints nothing.
- Given `src/store/` and `src/network/router.rs`, when they are inspected, then neither imports the other.
- Given Postgres stopped, when `make check` runs, then it succeeds and `git status --porcelain .sqlx` is empty.
- Given the regenerated `.sqlx/` committed and Postgres stopped, when `make image` runs, then the build succeeds.
- Given the committed tree and Postgres up, when `make check-sqlx` runs, then it exits 0; and given one store query edited without `make prepare`, it exits non-zero.
- Given a running gateway, when `make smoke` and `make probe` run, then both pass as before.
- Given `RUST_LOG=warn` and a ready gateway, when Postgres stops, then one `WARN` line shows the readiness change with the database error, and no further readiness lines appear while it stays down.

## Implementation Notes

- Committed as `c5dafce` on `feat/1-2-store-boundary`, unpushed. The implementation subagent committed without being asked.
- `list_routes` uses `query_as!(StoredRoute, …)`. It compiles because all three columns are `NOT NULL`.
- `.sqlx/`: the INSERT file `fdaf31…` was replaced by `46e03a…` (whitespace cleanup). The SELECT and DELETE hashes are unchanged.
- Verified by the subagent, against live Postgres and a running gateway:
  - `make check-sqlx` exits 0 on the committed tree. After an edit to the DELETE text it exits non-zero, listing one deleted and one untracked file.
  - `make image` builds with Postgres stopped.
  - `make smoke` and `make probe` pass.
  - With Postgres down, a create returns `Internal` "Database persistence failure: pool timed out…", and `GetGatewayStatus` then lists no route.
  - With `RUST_LOG=warn`, one readiness `WARN` with `error=` appears, then nothing for about 45s.
- Re-checked in the main session: the boundary greps print nothing, and `make check` passes offline with `.sqlx/` clean.
- Matrix test audit: no row has an automated test. The repo has no test harness, and every row needs a live or stopped Postgres. All four rows were covered by the live manual runs above. The first tests are expected with Story 1.5 (see `deferred-work.md`).
- Behaviour notes:
  - Readiness reaches NOT_SERVING about 30s after Postgres stops, because of sqlx's default acquire timeout. This is unchanged; Story 1.5 sets the timeout.
  - A start with the database down now logs the first readiness transition at `WARN`, where it used to be `info`.
- Not exercised: a kind redeploy (`make load` plus a rollout restart).

## Spec Change Log

## Review Triage Log

Pass 1 (loop 0). Sources: B = blind-hunter, E = edge-case-hunter, V = verification-gap. The blind hunter read the spec and `deferred-work.md` although it was given only the diff, so its findings are not blind. Every finding was still verified.

| # | Source | Finding | Verdict | Evidence | Route |
|---|--------|---------|---------|----------|-------|
| 1 | V | Create's store-first order has no automated test | gap | V, pre-verified: no `#[test]` anywhere; the smoke client never makes the upsert fail. | defer (group A) |
| 2 | V | The not-ready `WARN` and its once-per-transition throttle have no automated test | gap | V, pre-verified: nothing captures log output; `health_probe` checks status only. | defer (group A) |
| 3 | B | `Store` is concrete, so failure paths are only testable against a real stopped Postgres | medium | Real: no trait or test constructor exists. Pre-existing: `PgPool` was just as concrete before. This is the root cause of #1 and #2. | defer (group A) |
| 4 | V, E, B | Concurrent create/create or create/delete on one IP interleave between store and cache, leaving them disagreeing until restart | medium | Real: no lock spans the store call and the cache update. Pre-existing: the old order had the mirror race (cache then DB). `replicas: 1` does not prevent it within one pod. | defer (group B) |
| 5 | E | A cancelled request future can be dropped between a committed store write and its cache update | low | Same root cause as #4: the store write and the cache update are not one unit. The old create order had the mirror split; delete's order is unchanged. | defer (group B) |
| 6 | E | A create committed between `hydrate`'s `list_routes` and `load_routes` is wiped from the cache for good | medium | Real: `load_routes` replaces the table and `hydrated=true` prevents a reload. The server accepts calls while not ready. Pre-existing: same SELECT-then-replace before the move. | defer |
| 7 | E | An empty or over-long `tunnel_id`/`remote_endpoint` reaches the upsert and surfaces as `Internal` with raw sqlx text | medium | Real, pre-existing (no validation before or after). Story 1.5 owns the bounds and the taxonomy. | defer |
| 8 | B | `make check-sqlx`'s failure message says "run make prepare", which the target has just run | low | Real: the recipe's first line is the same `cargo sqlx prepare` as `make prepare`. A direct text correction. | patch |
| 9 | B | `check-sqlx` rewrites `.sqlx/`, so a second run passes with nothing committed | false | The regenerated files stay uncommitted, so `git status --porcelain -- .sqlx` is still non-empty and the second run fails too. Failing on correct-but-uncommitted data is the AC ("regenerates .sqlx/ and fails on git diff"). | reject |
| 10 | E | If `git status` itself fails, the empty substitution makes `check-sqlx` pass | low | Real in principle, but it needs git missing or a non-repo checkout on a dev box that clones with git. The fix adds a guard. | reject |
| 11 | B | `upsert_route`/`delete_route` take an unchecked `&str`, so writes are not tied to the parsed address | false | Both handlers write only after `parse::<Ipv4Addr>()` succeeds. Rust's parser is strict (checked here: leading zeros, whitespace, hex and short forms all fail), so the written string equals `local_ip.to_string()`. No bad row can be written today. | reject |
| 12 | B | Ordering is still a per-handler convention, not enforced by the type | low | True, but the AC defines the boundary as the store never calling the cache, and that holds. No caller breaks it today; enforcing more needs a new service layer. | reject |
| 13 | B | The readiness `WARN` does not name the failing check (hydrate vs ping), and a cause change mid-outage is not logged | low | The hydrate case is the one with no prior `hydrated N route(s)` line, so it can be told apart. Logging once per transition is the user's decision. The fix adds a field. | reject |
| 14 | B | `deferred-work.md` still lists the readiness and failed-create entries as open | low | Real, but this workflow keeps `deferred-work.md` append-only ("Do not modify existing entries"). The closure is recorded in this spec's Boundaries. | reject |
| 15 | B | `AGENTS.md` still says `make check` is the only automated verification, and has no store rule | low | Real: `make check-sqlx` now exists. The fix edits an agent-context file; Story 1.7 owns the refresh. | defer |
| 16 | V | `epic-1-context.md` says `check-sqlx` uses `git diff --exit-code` | false | The context mirrors the epic AC. The spec deviates on purpose, and the Makefile comment explains why (`diff` misses untracked files). No behaviour is wrong. | reject |
| 17 | V | `check-sqlx` is not run by any other target | false | By design: spine AD-4 as applied keeps it separate because it needs Postgres, and `make check` must stay offline. | reject |

## Verification

**Commands:**
- `make check` -- expected: no new warnings, `.sqlx/` untouched.
- `grep -rn sqlx src/ --exclude-dir=store` -- expected: no output.
- `make db-up && make migrate && make check-sqlx` (after committing) -- expected: exit 0.
- `make db-down && make image` -- expected: build succeeds.

**Manual checks:**
- `make db-up && make run`; `make smoke` and `make probe` pass, and the `hydrated N route(s) from the database` line appears.
- `make db-down`, then a create returns `Internal` with the same message as before, and `GetGatewayStatus` lists no route for that IP.
