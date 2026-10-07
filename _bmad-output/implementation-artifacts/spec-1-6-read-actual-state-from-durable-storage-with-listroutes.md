---
title: 'Read actual state from durable storage with ListRoutes'
type: 'feature'
created: '2026-10-07'
status: 'done'
baseline_commit: '1d3f07ea30d582a9abc53b9ea39863c9d87aeca9'
route: 'dispatch'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The only read surface, `GetGatewayStatus`, serves one pod's in-memory cache, so a reconciler would see different "actual state" depending on which pod answered and whether it hydrated, and could read a database outage as an empty table (FR-3, AD-1, AD-12).

**Approach:** Add a `ListRoutes` RPC that returns every `vpn_routes` row through the existing `Store::list_routes`, as a new three-field `Route` message, with store failures mapped by Story 1.5's `From<store::Error> for Status`. Add a `list_routes` example client and a `make routes` target to print actual state.

## Boundaries & Constraints

**Always:**
- `ListRoutes` reads the store only; it never reads or mutates the `RoutingTable`.
- `Route` has exactly `local_ip = 1`, `tunnel_id = 2`, `remote_endpoint = 3`, copied verbatim from the row (no parsing, no filtering — a row with a malformed `local_ip` is still returned). `id`, `created_at`, `owner` are never on the wire.
- Request is an empty `ListRoutesRequest`; response is `ListRoutesResponse { repeated Route routes = 1; }`. Order is unspecified and the proto says so; callers key on `local_ip`.
- A store error returns `Err(Status)` via `Status::from` — never `Ok` with an empty list. Logged once with `code` (error level for `UNAVAILABLE`/`INTERNAL`, warn otherwise), matching the write paths.
- A successful call logs at `debug` with the route count: the operator polls it every pass, so `info` would flood the log.
- No SQL text change: `.sqlx/` stays byte-identical; `make check` and `make test` stay offline.

**Never:**
- No change to `RouteDetails`, `GetGatewayStatus`, `RoutePacket`, the store's SQL, the schema, or `replicas: 1`.
- No AGENTS.md edit (1.7), no re-hydration, no pagination or filtering, no gRPC reflection.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Rows present | N rows in `vpn_routes` | OK, N `Route`s with stored strings | N/A |
| Empty table | DB reachable, 0 rows | OK, empty `routes` | N/A |
| DB down | pool cannot connect | `UNAVAILABLE` in ~5s; cache untouched | logged with `code` |
| Cache disagrees | route cached but not in DB (or vice versa) | result follows the DB | N/A |
| Never-hydrated pod | cache empty, rows in DB | every row returned | N/A |

</frozen-after-approval>

## Code Map

- `proto/gateway.proto` -- add `rpc ListRoutes`, `ListRoutesRequest`, `ListRoutesResponse`, `Route`. Extend the service comment: the four codes govern `ListRoutes` too (it can return `UNAVAILABLE`/`INTERNAL`); an error is never an empty list; `GetGatewayStatus` is a pod-local debug view, `ListRoutes` the authority. Leave `RouteDetails` untouched.
- `src/store/mod.rs:73-80` -- `Store::list_routes()` already returns `Vec<StoredRoute>` with exactly the three columns (used by `hydrate`). Reuse unchanged.
- `src/services/gateway.rs` -- new `list_routes` handler in `impl GatewayController for Gateway`. `proto::Route` collides with the imported `network::router::Route`: refer to it qualified. Reuse `Status::from(err)`; do not call `log_failed_write` (it takes `local_ip`/`owner`).
- `src/services/gateway.rs` tests -- reuse `gateway_with_db_down`, `cache_old_route`, `assert_old_route_kept`, `assert_unavailable`.
- `examples/list_routes.rs` (new) -- modelled on `examples/health_probe.rs` (`GATEWAY_ADDR`, default `http://127.0.0.1:50051`). Sort by parsed `Ipv4Addr`, unparseable rows last by raw string; print one aligned line per route, then `N route(s)`. An RPC error propagates through `?` (non-zero exit, code printed), so an outage is never shown as `0 route(s)`. `println!` is fine in `examples/`.
- `examples/smoke_client.rs` -- add `ListRoutes` assertions; update the doc comment.
- `Makefile` -- `routes` target beside `smoke`/`probe`: `SQLX_OFFLINE=true cargo run --quiet --example list_routes`.

## Tasks & Acceptance

**Execution:**
- [x] `proto/gateway.proto` -- RPC, three messages, comments -- the wire contract Epic 3 builds on.
- [x] `src/services/gateway.rs` -- `list_routes` handler: store, map rows, log -- the authoritative read.
- [x] `src/services/gateway.rs` tests -- (a) DB down: `UNAVAILABLE`, cached route kept and not returned; (b) DB down with an empty cache: still `UNAVAILABLE`, never `Ok(empty)`.
- [x] `examples/list_routes.rs`, `Makefile` -- client and `make routes` -- how people (and Stories 3.2, 4.1, 7.2) inspect actual state.
- [x] `examples/smoke_client.rs` -- after the first create, `ListRoutes` contains exactly `Route { "10.0.1.5", "tun-paris-01", "203.0.113.7:51820" }`; after the own update, the `tun-paris-02` values; after the first delete, no route for `10.0.1.5`.

**Acceptance Criteria:**
- Given Postgres stopped, when `make check` and `make test` run, then both pass and `git status --porcelain .sqlx` is empty.
- Given `make run` against a live database, when `make smoke` runs, then it passes including the `ListRoutes` steps.
- Given `make db-reset` and `make run`, when `make routes` runs, then it prints `0 route(s)` and exits 0; given Postgres then stopped, it exits non-zero naming `Unavailable`.
- Given gateway A (`make run`) and gateway B started afterwards on another `BIND_ADDR` against the same database, when a tunnel is created through A, then `GATEWAY_ADDR=http://127.0.0.1:50052 make routes` lists it, while B's `GetGatewayStatus` does not.

## Implementation Notes

- The handler logs a store failure inline (error for `UNAVAILABLE`/`INTERNAL`, warn otherwise) instead of through `log_failed_write`, which needs `local_ip`/`owner`.
- `examples/smoke_client.rs` asserts the whole `Vec<Route>` filtered to `10.0.1.5`, so a duplicate row would also fail. Its helper is `expected_route`, not `route`, because the existing `let route = ...` bindings would shadow it.
- Verified live on 2026-10-07: `make smoke` passes; `make routes` prints `0 route(s)` against an empty table (it was already empty, so `make db-reset` was not run) and exits 2 with `Unavailable` about 5s after `make db-down`. Two gateways (B on `127.0.0.1:50052`, hydrated before A's creates): B's `GetGatewayStatus` held 0 routes while `GATEWAY_ADDR=http://127.0.0.1:50052 make routes` listed all 3, sorted numerically. A hand-inserted `local_ip = 'not-an-ip'` row was returned verbatim and listed last.
- Matrix coverage: "DB down" by the two offline unit tests; "Rows present" and "Cache disagrees" (DB-only after delete) by `make smoke`; "Empty table" and "Never-hydrated pod" by the live checks above. No offline test reaches a working store — the harness gap deferred in 1.4/1.5.
- Correction (review #12): `make smoke` does not cover "Cache disagrees" — its cache and DB agree at every ListRoutes check. That row, like "Never-hydrated pod", is covered only by the manual two-gateway check.

## Spec Change Log

## Review Triage Log

Pass 1 (loop 0). Sources: B = blind-hunter, E = edge-case-hunter, V = verification-gap.

| # | Source | Finding | Verdict | Evidence | Route |
|---|--------|---------|---------|----------|-------|
| 1 | E, B, V | The proto says ListRoutes returns only `UNAVAILABLE`/`INTERNAL`, but `Status::from` maps a SELECT's SQLSTATE class 22 (e.g. `22P05`) to `INVALID_ARGUMENT`; the handler's `warn!` arm expects that | low | Real: `classify_sqlstate` is statement-agnostic. Rare on a SELECT; the fix is rewording the comment ("any non-OK status means actual state is unknown, never empty"). | patch |
| 2 | E, B | One unpaged response hits tonic's 4 MiB client decode limit at roughly 7-8k max-width rows; undocumented | low | Real arithmetic (45+255+255 chars plus framing). Pagination is excluded by the frozen Never list; documenting the limit is a direct comment correction. | patch |
| 3 | E | A SELECT that hangs after acquire (lock wait) is unbounded, so the polling reconciler hangs or sees `DEADLINE_EXCEEDED` | medium | Real and pre-existing: the 1.5 statement_timeout deferral covers every statement; ListRoutes now also relies on it. | defer |
| 4 | E | `make routes` prints stored strings raw, so a `tunnel_id` with CR/LF/ANSI escapes breaks columns or reaches the terminal | low | Real: `validate_tunnel_id` checks length only (1.5 #2). New printer in this diff; `escape_debug` per field is direct. | patch |
| 5 | E | Column width counts chars, so wide/zero-width Unicode misaligns | low | Cosmetic, unlikely, fix adds a dependency. | reject |
| 6 | B | Smoke checks only the cache after the foreign-owner writes and the refused invalid create, so an intruder overwriting the row passes | medium | Real: `examples/smoke_client.rs` asserts `GetGatewayStatus` there; the proto now says the cache is not the authority. Two `listed_routes` asserts. | patch |
| 7 | B | The proto never states the AD-12 comparison fields or that `local_ip` is unique | low | Real: `Route` comment says "compare by exact string" without fields; uniqueness is a schema fact smoke relies on. Direct comment correction. | patch |
| 8 | B, V | No offline test runs the row-to-`Route` copy; a filter (hydrate-style parse skip) or normalisation or field swap passes every check | medium | V pre-verified: both unit tests `expect_err` before the mapping; smoke rows are always canonical IPv4. Extract a private fn and test it offline. | patch |
| 9 | B | `list_routes_with_db_down_and_empty_cache_is_never_an_empty_list` cannot fail when the first test passes | low | Real: the first test's `expect_err` already fails on any `Ok`; its last assert is on a never-filled cache. Deletion. | patch |
| 10 | B | `make help` text for `smoke` still says create/observe/delete | low | Real; direct text correction. | patch |
| 11 | B | `examples/list_routes.rs` has no connect/RPC timeout | low | A stall needs the post-acquire hang (#3); acquire is bounded at 5s and a closed port fails fast. Fix adds endpoint configuration. | reject |
| 12 | V | No repeatable check that ListRoutes follows the DB when the cache differs on the success path; the Implementation Notes wrongly credit `make smoke` with "Cache disagrees" | medium | V pre-verified: smoke's cache and DB agree at every ListRoutes check. Needs the DB-backed harness deferred in 1.4/1.5. The notes are corrected by an appended line. | defer |

## Verification

**Commands:**
- `make db-down; make check; make test` -- expected: clean, all tests pass, `.sqlx/` untouched.
- `make db-up && make check-sqlx` -- expected: passes with no `.sqlx/` change.
- `make run` then `make smoke` and `make routes` -- expected: smoke OK line; routes prints a count.
- `grep -rn sqlx src/ --exclude-dir=store; grep -rn println src/` -- expected: no output.

**Manual checks:**
- The two-gateway check in the last AC (B hydrates before A's create, so B's cache lacks the route).
