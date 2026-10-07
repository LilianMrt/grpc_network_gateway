---
title: 'Give every route an owner and scope writes to it'
type: 'feature'
created: '2026-10-07'
status: 'done'
baseline_commit: '269db4f70efd8993d88d2def94930a25deb5d427'
route: 'dispatch'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** `vpn_routes` rows have no owner, so two `VpnTunnel` resources declaring one `local_ip` upsert over each other forever, and a delete in one namespace destroys another namespace's tunnel (AD-13, FR-34).

**Approach:** Add `owner` (`<namespace>/<name>`) to the seed schema and to `TunnelRequest`/`DeleteTunnelRequest`. Create writes only a missing row or the caller's own, atomically in one statement; delete removes only the caller's own row. Empty `owner` is `INVALID_ARGUMENT` before any write.

## Boundaries & Constraints

**Always:**
- `owner TEXT NOT NULL` in both schema copies (`migrations/01_init_routing_table.sql`, `k8s/12-configmap-initdb.yaml`), identical, same commit, edited in place — no new migration.
- Ownership is decided inside the SQL statement (conditional upsert, owner-scoped delete), never by a read-then-write in Rust, so two concurrent creates cannot both win.
- Store first, cache second, on both handlers. A conflicting create and a delete that removed nothing leave the routing table untouched.
- Validation order: `local_ip`, then `owner`; both before any store call.
- `.proto` edit, `.sqlx/` regeneration and the SQL edit land in the same commit.
- DB-failure statuses stay `INTERNAL` with today's messages (1.5 changes them); `make check` and `make test` stay offline.
- `owner` never appears in `GetGatewayStatus`, `RouteDetails`, or the routing table.

**Never:**
- No error taxonomy work beyond `FAILED_PRECONDITION` for the conflict (1.5), no `ListRoutes` (1.6), no AGENTS.md edit (1.7), no reaper.
- The conflict message never names the other owner (it crosses a namespace boundary).
- The kind Postgres PVC is not touched during implementation.

**Decisions (2026-10-07, user):**
- Owner shape: only the empty string is rejected. No `<namespace>/<name>` format check and no length bound in this story.
- Conflict proof: `smoke_client` sends a foreign-owner create (expects `FAILED_PRECONDITION`) and a foreign-owner delete (expects `success=true, existed=false`, route still in `GetGatewayStatus`) before its own deletes, so `make smoke` guards AD-13.
- Resets: add `make db-reset` and `make cluster-db-reset`; run `db-reset` locally during implementation (local dev rows are disposable).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Create, no row | owner `a/x` | row inserted with owner `a/x`, cached | N/A |
| Create, own row | row owned `a/x`; create `a/x`, new tunnel | row and cache updated | N/A |
| Create, foreign row | row owned `a/x`; create `b/y` | `FAILED_PRECONDITION`; row and cache unchanged | logged with `local_ip`, `owner`, `code` |
| Delete, own row | row owned `a/x`; delete `a/x` | `success=true, existed=true`; row and cache removed | N/A |
| Delete, foreign row | row owned `a/x`; delete `b/y` | `success=true, existed=false`; row and cache unchanged | N/A |
| Delete, no row | delete `a/x` | `success=true, existed=false` | N/A |
| Empty owner | create or delete with `owner=""` | `INVALID_ARGUMENT` | no store call |

</frozen-after-approval>

## Code Map

- `proto/gateway.proto:11-24` -- add `string owner = 4;` to `TunnelRequest`, `string owner = 2;` to `DeleteTunnelRequest`; extend the delete comment: idempotent within the caller's ownership.
- `migrations/01_init_routing_table.sql`, `k8s/12-configmap-initdb.yaml:16-22` -- add `owner TEXT NOT NULL`.
- `src/store/mod.rs:66-91` -- `upsert_route` gains `owner`, uses `ON CONFLICT (local_ip) DO UPDATE ... WHERE vpn_routes.owner = EXCLUDED.owner`; `rows_affected() == 0` means another owner holds it. Return a new `pub enum UpsertOutcome { Written, OwnedByAnother }`. `delete_route` gains `owner`: `WHERE local_ip = $1 AND owner = $2`. `list_routes` unchanged.
- `src/services/gateway.rs:68-173` -- owner check after the `local_ip` parse in both handlers; create maps `OwnedByAnother` to `Status::failed_precondition` (warn line with `local_ip`, `owner`, `code`); delete calls `remove_route` only when `existed`; delete message for `existed=false` keeps "nothing to delete". Add `owner` field to the existing create/delete log lines.
- `src/services/gateway.rs:238-337` -- tests: existing request literals gain an owner; `Store::unreachable()` from 1.3 reused.
- `examples/smoke_client.rs` -- fixed owner `smoke/smoke-client` on create and both deletes.
- `Makefile` -- `db-reset`: `docker compose down -v`, `up -d`, wait on `docker compose exec -T postgres pg_isready -h 127.0.0.1` (TCP: the image's init-phase server listens on the socket only, so a socket check passes too early), then `sqlx migrate run`. `cluster-db-reset`: delete StatefulSet `postgres` and PVC `data-postgres-0` in `netgw` (`--ignore-not-found`), `kubectl apply -f k8s/`, `kubectl rollout restart deployment/gateway -n netgw` (the gateway hydrates only at startup).
- `.sqlx/` -- regenerated by `make prepare`; old upsert/delete query files disappear, new ones appear.

## Tasks & Acceptance

**Execution:**
- [x] `proto/gateway.proto` -- add the two `owner` fields.
- [x] `migrations/01_init_routing_table.sql`, `k8s/12-configmap-initdb.yaml` -- add the column, identically.
- [x] `src/store/mod.rs` -- owner-scoped upsert returning `UpsertOutcome`, owner-scoped delete.
- [x] `src/services/gateway.rs` -- empty-owner rejection, conflict mapping, conditional cache removal, log fields.
- [x] `src/services/gateway.rs` tests -- empty owner on create and delete returns `InvalidArgument` against the unreachable store (proves no write was attempted: it would be `Internal`), table unchanged; existing three tests still pass.
- [x] `examples/smoke_client.rs` -- owner `smoke/smoke-client` on its create and both deletes; foreign-owner (`smoke/intruder`) create and delete steps per the decision, with assertions and `println!` lines matching the existing style.
- [x] `Makefile` -- `db-reset` and `cluster-db-reset` as in the Code Map, with `##` help text.
- [x] `make db-reset`, then `make prepare`; `.sqlx/` goes in the same commit as the SQL and proto edits.

**Acceptance Criteria:**
- Given a reset local Postgres, when `make check-sqlx` runs, then it passes with the regenerated `.sqlx/` committed.
- Given Postgres stopped, when `make check` and `make test` run, then both pass and `.sqlx/` is untouched.
- Given `make run` against the reset database, when `make smoke` runs, then it passes.
- Given both schema files, when diffed by column, then they define identical columns.

## Implementation Notes

## Spec Change Log

## Review Triage Log

Pass 1 (loop 0). Sources: B = blind-hunter, E = edge-case-hunter, V = verification-gap.

| # | Source | Finding | Verdict | Evidence | Route |
|---|--------|---------|---------|----------|-------|
| 1 | B | A foreign-owned delete answers "No tunnel for {ip}", which is false while the route is live | low | Real: the row exists under another owner. A direct text correction that names only the caller's owner keeps the conflict message rule. | patch |
| 2 | B | The refusal log names only the requester, so finding the holder needs psql | low | Real, but naming the holder needs a `RETURNING` or extra read on the refusal path; the intent keeps `owner` out of status reads, and 1.6 `ListRoutes` is the planned durable read. | reject |
| 3 | B, E | Owner shape is unenforced: whitespace, padding, `a/b/c`, unbounded `TEXT` | false | The 2026-10-07 user decision rejects only the empty string, with no format check and no length bound in this story. | reject |
| 4 | B, E, V | A database seeded before this change hydrates and reports SERVING, then every write fails `INTERNAL` | medium | Real: `list_routes` and the readiness `SELECT 1` never touch `owner`. The root is the pre-existing schema model (migration 01 edited in place, initdb only on an empty PVC); the intent chose the reset targets as the remedy. | defer |
| 5 | B, V | No test exercises `OwnedByAnother`, the `if existed` guard, or two concurrent creates against Postgres | medium | V pre-verified: all unit tests use `Store::unreachable()`; smoke is sequential, so a read-then-write regression passes every check. Pre-existing lack of a DB-backed harness; `make test` stays offline by constraint. | defer |
| 6 | B | AGENTS.md still prescribes `make migrate` after SQL changes and omits the reset targets and owner scoping | medium | Real, but an agent-context file; Story 1.7 owns the refresh. | defer |
| 7 | B | `cluster-db-reset` restarts the gateway before Postgres is ready and returns while it is NotReady | low | Real; the readiness task retries hydration, so it converges, but an immediate smoke fails. Two `rollout status` lines. | patch |
| 8 | B, E | `db-reset`'s `pg_isready` loop has no bound | low | Real: a container that never starts hangs the target forever. A bounded loop is a direct correction. | patch |
| 9 | E | `db-reset` while `make run` is up leaves wiped routes cached | low | Real: the gateway hydrates only at startup. A comment note is the direct correction. | patch |
| 10 | B | A failed smoke run leaves `10.0.1.5` owned by the intruder, failing later runs | low | Only reachable after an ownership regression the first failing run already reported; the fix adds cleanup steps. | reject |
| 11 | B | The first create's result is never asserted | false | `?` turns any error status into a smoke failure, and the handler returns `success=true` on every `Ok`. | reject |
| 12 | E | Delete no longer clears a cached route whose row is gone, so retries never heal cache/DB drift | low | Real, but drift needs the #13 race or out-of-band DB edits; distinguishing absent from foreign needs another query. Noted in #13's deferral. | reject |
| 13 | E | Concurrent create and delete on one `local_ip` can finish store and cache writes in different orders | medium | Real and pre-existing: handlers never serialized store+cache per key before this story. Since #12, a retried delete no longer heals the result. | defer |

## Verification

**Commands:**
- `make db-down; make check; make test` -- expected: clean, all tests pass, `git status --porcelain .sqlx` empty.
- `make db-up && make check-sqlx` -- expected: passes after the regenerated files are committed (staged is not enough: run after commit, or check `git status` shows only intended changes).
- `make run` then `make smoke` -- expected: OK line.
- `grep -rn sqlx src/ --exclude-dir=store; grep -rn println src/` -- expected: no output.

**Manual checks:**
- Swap create's store call and `add_route`, run `make test`, see failure, revert (1.3's guard still holds).
