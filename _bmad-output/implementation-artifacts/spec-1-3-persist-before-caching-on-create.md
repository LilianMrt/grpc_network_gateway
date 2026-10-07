---
title: 'Persist before caching on create'
type: 'bugfix'
created: '2026-10-07'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'd96a7ff221f0b3fb01ea1841f8a94b3dcd2572de'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Story 1.2 moved create's `add_route` after the upsert, but nothing proves it. A failed write leaving a live route (FR-31) was only checked by stopping Postgres by hand, and the repo has no test that would catch a regression.

**Approach:** Add the crate's first automated tests: handler-level unit tests that run create and delete against a store that cannot reach Postgres, and assert the routing table and `GetGatewayStatus` are unchanged. Expose them through a `make test` target that needs no database.

## Boundaries & Constraints

**Always:**
- Tests run offline: no Postgres, no running gateway, `SQLX_OFFLINE=true`. `make test` must pass with Postgres stopped.
- The failing store is a real `Store` whose pool points at a refused port with a short acquire timeout, so the error is the same `PoolTimedOut` production sees. The test-only constructor is `#[cfg(test)]`; production pool settings do not change.
- Tests call the `GatewayController` trait methods on a `Gateway` directly. No server, no tonic transport.
- gRPC responses, status codes, messages, and the 1.1 log lines stay byte-identical. Both handlers keep store first, cache second.
- `make check` stays offline and never writes `.sqlx/`.

**Never:**
- No production acquire timeout (Story 1.5), no error taxonomy change (1.5), no ownership (1.4), no `ListRoutes` (1.6), no AGENTS.md edit (1.7).
- No `Store` trait or mock layer; no new dependency.
- No SQL change, so `.sqlx/` is untouched.
- Decision (2026-10-07, user): concurrent store-and-cache writes on one `local_ip`, and a request cancelled between the two awaits, stay deferred (1.2 triage #4/#5). No write lock and no `tokio::spawn` in this story.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Create, DB down, empty cache | valid request for `10.0.0.5` | `Err`, code `Internal`, message starts `Database persistence failure` | no route for `10.0.0.5` in the table; `GetGatewayStatus` returns no route for it |
| Create, DB down, IP already cached | cache holds `10.0.0.5 → tun-old`; create `10.0.0.5 → tun-new` | `Err` `Internal` | cache still holds `tun-old` with its endpoint |
| Delete, DB down, IP cached | cache holds `10.0.0.5` | `Err`, code `Internal`, message starts `Database delete failure` | route still in the table |
| Create, DB up | valid request | row upserted, then cached | N/A (covered by `make smoke`) |

</frozen-after-approval>

## Code Map

- `src/services/gateway.rs:94-114` -- create: upsert at 98, `add_route` at 114. Already store first; do not reorder. Add `#[cfg(test)] mod tests` at the end of this file.
- `src/services/gateway.rs:145-158` -- delete: `delete_route` at 146, `remove_route` at 158. Already store first.
- `src/services/gateway.rs:216` -- `get_gateway_status` reads only the cache; call it in the tests through the trait.
- `src/store/mod.rs:38-41` -- `connect_lazy`. Add a sibling `#[cfg(test)] pub(crate) fn unreachable() -> Self` with `acquire_timeout` of about 200ms and URL `postgres://netgw@127.0.0.1:1/netgw`.
- sqlx-core 0.9.0 `pool/inner.rs:325-397` -- `ConnectionRefused` is retried with backoff until the acquire deadline, then `PoolTimedOut`. That is why the timeout must be short: the default 30s would make each test take 30s.
- `src/network/router.rs` -- `RoutingTable::new`, `add_route`, `lookup_route`; reuse for seeding and asserting.
- `Cargo.toml` -- tokio `full` already provides `#[tokio::test]`; no change.
- `Makefile:41` -- add `test` beside `check`.

## Tasks & Acceptance

**Execution:**
- [x] `src/store/mod.rs` -- add the `#[cfg(test)]` `unreachable()` constructor described in the Code Map, with a doc line saying why the timeout is short.
- [x] `src/services/gateway.rs` -- add a tests module covering the three DB-down rows of the matrix. Assert on `Status::code()` and the message prefix, and on the table through `lookup_route` and `get_gateway_status`. Add a one-line "store first, cache second" comment above delete's `delete_route`, matching create's.
- [x] `Makefile` -- `test: ## Run the unit tests (no database needed)` running `SQLX_OFFLINE=true cargo test`.
- [x] `_bmad-output/implementation-artifacts/deferred-work.md` -- append one entry: AGENTS.md still says there is no test suite and `make check` is the only automated verification; Story 1.7 owns the refresh.

**Acceptance Criteria:**
- Given Postgres stopped, when `make test` runs, then every test passes in under 10 seconds total.
- Given create's two lines swapped (`add_route` before the upsert), when `make test` runs, then the create tests fail.
- Given delete's two lines swapped, when `make test` runs, then the delete test fails.
- Given a running gateway against live Postgres, when `make smoke` runs, then it passes as before.

## Implementation Notes

- Uncommitted on `feat/1-3-persist-before-cache`.
- `Store::unreachable()` also sets `max_connections(1)`. With the 200ms timeout, the tests get `pool timed out while waiting for an open connection`, the production text.
- Verified by the subagent: `make test` 3/3 with Postgres stopped in 0.20s; swapping create's order fails both create tests, and swapping delete's fails the delete test; `make check` clean; the boundary and `println!` greps print nothing; `make smoke` passed against live Postgres.
- Re-checked in the main session: `make test` 3/3 in 0.43s wall with Postgres stopped; `make check` clean, `.sqlx/` untouched; the greps print nothing; the create swap fails both create tests.
- Matrix test audit: the three DB-down rows each have a test that ran and passed. The DB-up row is covered by `make smoke`, as the matrix says.
- The tests assume nothing listens on `127.0.0.1:1`.
- Review pass 1 patches (#5, #6, #10) applied; `make test` 3/3, `make check` clean, `.sqlx/` untouched, greps empty after the patch.

## Spec Change Log

## Review Triage Log

Pass 1 (loop 0). Sources: B = blind-hunter, E = edge-case-hunter, V = verification-gap.

| # | Source | Finding | Verdict | Evidence | Route |
|---|--------|---------|---------|----------|-------|
| 1 | B | The spec file is missing from the review diff | false | Excluded on purpose: the spec is the claims file, given to the edge-case layer alone. It is not code under review. | reject |
| 2 | B, V | `sprint-status.yaml` says `in-progress` while the spec says `in-review` | false | Step 5 syncs the story to `review` before the commit; the gap is mid-workflow, not a final state. | reject |
| 3 | B | The tests accept any store error, not specifically `PoolTimedOut` | false | The property under test is "any failed write leaves the cache unchanged", which a different store error proves equally. Pinning sqlx's message text would be classifying by string, and Story 1.5 changes these messages. | reject |
| 4 | B | The port-1 caveat is only in the spec, so a listener on that port gives a confusing failure | false | A non-Postgres listener makes sqlx fail at once; a silent drop is bounded by the 200ms acquire timeout. Either way the store fails and the tests still pass for the right reason. | reject |
| 5 | B | The delete test checks less than the create test (no `remote_endpoint`, `is_some()` only) | low | Real: a regression that rewrote the endpoint on failed delete would pass. The fix is two assertions. | patch |
| 6 | B | The new delete comment points at create instead of stating delete's consequence | low | Real: the comment carries no reason. A direct text correction. | patch |
| 7 | B | The new deferred-work entry repeats the 1.2 #15 entry | low | Overlap is real, but `deferred-work.md` is append-only by workflow rule; merging would edit an existing entry. | reject |
| 8 | B | No handler-level test covers the success path (upsert then `add_route`) | medium | Real: dropping `add_route` after a good upsert passes `make test`. `make smoke` does check `GetGatewayStatus` after create, but needs live Postgres and is run by hand. A unit test needs a working store, which the spec rules out here. | defer |
| 9 | E | Concurrent create/delete on one IP leaves cache and DB disagreeing | medium | Real and pre-existing; the user decided on 2026-10-07 to keep it deferred, and it is already in `deferred-work.md` (1.2 #4/#5). | reject |
| 10 | E | The empty-table test asserts only on 10.0.0.5, not the whole table | low | Real: caching under another key would pass a test named for an empty table. One assertion. | patch |
| 11 | V | Nothing in AGENTS.md, epic-1-context, or the 1.7 AC tells later stories to run `make test`, so 1.4–1.6 can regress the ordering with every documented check green | medium | Pre-verified gap. The fix edits agent-context guidance (AGENTS.md; 1.7 owns it), or changes what `make check` means, which is the user's call. | defer |
| 12 | V | Code Map line numbers are stale after the added comment line | low | True (158 → 159), but the fix is a spec edit. | reject |

## Verification

**Commands:**
- `make db-down; make test` -- expected: all tests pass, quickly, with no database.
- `make check` -- expected: no new warnings; `git status --porcelain .sqlx` empty.
- `grep -rn sqlx src/ --exclude-dir=store` -- expected: no output.

**Manual checks:**
- Temporarily swap create's upsert and `add_route`, run `make test`, see the create tests fail, then revert. Repeat for delete.
- `make db-up && make run`, then `make smoke` passes.
