# Epic 1 Context: Durable actual-state reads

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

Make the Rust Gateway a control-plane API that an external reconciler (the Go operator, built in later epics) can trust. A caller can read the authoritative set of Tunnels from Postgres whichever pod answers. The same failure returns the same gRPC code from every RPC. No resource can overwrite or delete a Tunnel it does not own. Along the way this epic fixes the known defect where create writes memory before the database, moves all SQL behind one module, and replaces `println!` with structured `tracing`. The epic is useful on its own before any operator exists. It has grown past its original weekend budget.

## Stories

- Story 1.1: Emit every gateway log line through `tracing`
- Story 1.2: Move every SQL statement behind `src/store/`
- Story 1.3: Persist before caching on create
- Story 1.4: Give every route an owner and scope writes to it
- Story 1.5: Return one failure taxonomy from every write path
- Story 1.6: Read actual state from durable storage with `ListRoutes`
- Story 1.7: Bring `AGENTS.md` in line with the code E1 leaves behind
- Story 1.8: Bound what a tunnel identifier and an owner may contain *(added 2026-10-07)*
- Story 1.9: Expose route ownership on `ListRoutes` *(added 2026-10-07)*
- Story 1.10: Prove the store's SQL against a real database *(added 2026-10-07)*

**Amended 2026-10-07** by the Epic 1 retrospective and `planning-artifacts/sprint-change-proposal-2026-10-07.md`, which take precedence over older text below where they conflict:
- `Route` carries `owner` as a fourth field; "already correct" is `owner` equal to the caller's plus exact `tunnel_id`/`remote_endpoint` equality (AD-12 amended). `id` and `created_at` stay off the wire.
- `tunnel_id` matches `^[A-Za-z0-9][A-Za-z0-9._-]*$`, 1-255 characters. `owner` is non-empty, at most 317 characters, with no control characters (U+0000-U+001F, U+007F-U+009F); its format is otherwise unchecked (AD-15 amended). Bounds apply to writes only; existing rows still list and hydrate.
- `make test-db` runs `#[sqlx::test]` tests against Postgres; `make test` stays offline.
- Order: 1.8, then 1.9, then 1.10, all before Epic 2.

## Requirements & Constraints

- **Durable reads.** A new `ListRoutes` RPC returns every row of `vpn_routes`. Its result must not depend on which pod serves the call or on whether that pod ever hydrated. It never reads or mutates the in-memory routing table.
- **Error versus empty.** If the database is unreachable, `ListRoutes` returns `UNAVAILABLE`. If the database is reachable and the table is empty, it returns OK with an empty list. An error must never be rendered as an empty result, because the reconciler would read that as total drift and re-create everything.
- **Persist before cache, on every write path.** If the database write fails on create, the call reports no success, the routing table stays unchanged, and `GetGatewayStatus` shows no route for that IP.
- **Failure taxonomy, identical on every RPC:**
  - Unreachable database or pool error, including pool acquire timeout: `UNAVAILABLE` (retryable).
  - Invalid input: `INVALID_ARGUMENT`, raised before any write.
  - Any Postgres constraint violation (length, type, check): `INVALID_ARGUMENT`, never `INTERNAL`. It is treated as permanent.
  - Ownership conflict: `FAILED_PRECONDITION`.
  - `INTERNAL` only for genuinely unexpected failures.
  - Older PRD text that says a failed create returns `INTERNAL` is superseded by this taxonomy.
- **Ownership:**
  - Every row has exactly one owner, `<namespace>/<name>`.
  - An empty `owner` on create or delete returns `INVALID_ARGUMENT` before any write. The empty string is never treated as an owner.
  - Create claims a missing row or updates the caller's own row. On a row with a different owner, create returns `FAILED_PRECONDITION` and changes nothing.
  - Delete removes only rows the caller owns.
- **Idempotency is unchanged.** Create still upserts on `local_ip`. When there is nothing of the caller's own to delete, delete returns `success=true, existed=false`, never an error.
- **Validation bounds.** The Gateway enforces these, and they must be written down explicitly because the CRD validation must use the same numbers:
  - `local_ip` is IPv4.
  - `tunnel_id` and `remote_endpoint` are bounded at their column widths.
  - `remote_endpoint` has the form `host:port`.
- **Logging:**
  - Every log line goes through `tracing`, with `tracing-subscriber` initialised at startup. No `println!` may remain anywhere under `src/`.
  - Lines about create, delete and hydration carry `local_ip` as a structured field.
  - Error lines carry the gRPC code as a field.
  - Keep a line equivalent to `hydrated N route(s) from the database`; the pod-restart demo relies on it.
- **Liveness never depends on the database.** Moving the readiness `SELECT 1` must not change this.

## Technical Decisions

- **Source of truth.** `vpn_routes` is the only authority on whether a Tunnel exists. Each pod's routing table is a cache derived from it. `GetGatewayStatus` and `RoutePacket` read the cache and are debugging or demo surfaces only. `GetGatewayStatus` must never stand in for `ListRoutes`.
- **Store boundary:**
  - All SQL lives in `src/store/`: `sqlx::query!`, the function form `sqlx::query`, and any other path to Postgres. This includes the readiness probe's `SELECT 1`, which becomes `store::ping()`.
  - Handlers call the store first and the cache second, never the reverse.
  - The store never calls the cache, and `network/router.rs` neither knows about nor imports sqlx.
- **Error mapping location.** The mapping lives at the store boundary, in exactly one place, so that any RPC added later inherits it. Never classify an error by matching its message string.
- **Pool acquire timeout.** Set it explicitly where the pool is built. sqlx's default is 30s and none is set today. It must be shorter than the operator's default per-call deadline, so the operator sees `UNAVAILABLE` rather than its own `DEADLINE_EXCEEDED`.
- **`ListRoutes` wire shape:**
  - It returns `repeated Route`, a new message with exactly three fields: `local_ip`, `tunnel_id`, `remote_endpoint`.
  - Leave `RouteDetails` alone. It is keyed `destination_ip` and belongs to `GetGatewayStatus`.
  - `id`, `created_at` and `owner` are bookkeeping columns and never go on the wire. The reconciler decides "already correct" by exact string comparison over `tunnel_id` and `remote_endpoint`.
- **Schema change for `owner`:**
  - Add `owner` as `TEXT NOT NULL`, not `VARCHAR(255)`, because `<namespace>/<name>` can be up to 317 characters.
  - Add it to the seed schema, not as a new `sqlx migrate` migration.
  - The schema has two copies, `migrations/01_init_routing_table.sql` and `k8s/12-configmap-initdb.yaml`. Both change in the same commit and must stay identical.
  - Existing databases need a documented reset: recreate the local compose volume (sqlx stores a checksum for migration 01), and delete and redeploy the kind Postgres PVC (init hooks never re-seed).
- **Generated artefacts:**
  - Edits to `.proto` and to SQL go in the same commit as their regenerated outputs.
  - `.sqlx/` is regenerated with `make prepare` against a database that already has the new column.
  - Add a new `make check-sqlx` target. It needs Postgres, regenerates `.sqlx/`, and fails on `git diff --exit-code`.
  - `make check` must keep working with Postgres stopped and must not touch `.sqlx/`. No CI exists; these Makefile checks are the enforcement.
- **Proto.** `TunnelRequest` and `DeleteTunnelRequest` gain an `owner` field. Update `examples/smoke_client.rs` to send a fixed owner on its create and on both deletes, and keep `make smoke` passing.
- **Example client.** Add a small `ListRoutes` client next to the smoke client and run it through a Makefile target. The Gateway serves no gRPC reflection, so this client is how people inspect actual state.
- **Out of scope:**
  - No reaper or sweep that deletes rows without a matching resource. An orphan row is a valid steady state.
  - No periodic re-hydration inside the Gateway.
  - No change to `replicas: 1`. Writes still update only the serving pod's cache.
  - No new fields beyond what is listed here.

## Cross-Story Dependencies

- **1.1 goes first** so that every line the later stories add is structured from the start.
- **1.2 comes before 1.3, 1.5 and 1.6.** It creates the store boundary that the write ordering, the error mapping and `ListRoutes` all depend on.
- **1.4 comes before 1.5**, which assigns `FAILED_PRECONDITION` to the ownership conflict that 1.4 introduces.
- **1.6 depends on 1.5's mapping** for its `UNAVAILABLE` result.
- **1.7 comes last.** It rewrites `AGENTS.md` once the other stories have landed:
  - `tracing` is the only logging path.
  - No SQL is allowed outside `src/store/`.
  - It records which check needs Postgres.
  - The kind node pin stays at `v1.33.1`, with a note that Stories 5.1 and 5.3 retire it.
- **Other epics depend on this one:**
  - Epic 2's CRD validation (Story 2.3) uses the bounds recorded in 1.5.
  - Epic 3 needs `ListRoutes`.
  - Stories 3.2, 4.1 and 7.2 reuse the `ListRoutes` example client rather than writing their own.
  - Story 4.4 relies on the pool timeout being shorter than Story 3.1's call deadline.
  - Story 5.2 packages the schema that includes `owner`.
  - Stories 5.2 and 5.3 require `make smoke` to pass.
