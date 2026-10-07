# Deferred Work

- source_spec: `_bmad-output/implementation-artifacts/spec-1-1-emit-every-gateway-log-line-through-tracing.md`
  summary: A create that fails at the database leaves its route live in the in-memory table, so RoutePacket forwards through a tunnel the client was told failed.
  evidence: Triage #7/#27. `add_route` runs before the INSERT in `create_vpn_tunnel`. Observed in a live run with Postgres stopped. Story 1.3 (persist before caching) owns the fix.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-1-emit-every-gateway-log-line-through-tracing.md`
  summary: The readiness task discards hydrate and SELECT 1 errors, and logs the NotServing transition only at info, so an outage has no logged cause and RUST_LOG=warn hides it.
  evidence: Triage #10/#26. `Err(_) => false` and `.is_ok()` in `src/services/health.rs` are unchanged from before the story. Natural home: Story 1.2, when the ping moves to `store::ping()`.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-1-emit-every-gateway-log-line-through-tracing.md`
  summary: Client-supplied strings (raw local_ip, tunnel_id, stored remote_endpoint) are logged with Display, which allows CR/LF log-line injection.
  evidence: Triage #5/#21. Pre-existing for tunnel_id via println!. Fixing it means using Debug (?) for unvalidated input, which needs the spec's "% for Display" rule revisited. Natural home: Story 1.5's validation bounds.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-1-emit-every-gateway-log-line-through-tracing.md`
  summary: CreateVpnTunnel logs no success line, and RoutePacket's MALFORMED_HEADER drop logs nothing, so both outcomes are invisible in the logs.
  evidence: Triage #8/#28 and #9/#29. There was no println! for either before the story.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-1-emit-every-gateway-log-line-through-tracing.md`
  summary: Fatal startup errors (bad BIND_ADDR, missing DATABASE_URL, bind failure) leave through main's ? on stderr, outside tracing, after an INFO "listening" line that is emitted before the bind.
  evidence: Triage #11/#24 and #12/#25. Both are the pre-existing order and error path in `src/main.rs`.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-1-emit-every-gateway-log-line-through-tracing.md`
  summary: No automated test covers the logging filter defaults (empty or invalid RUST_LOG), the four handler error Status values, the hydrate() unparseable-row arm, or the FR-10 "hydrated N route(s) from the database" text.
  evidence: Triage #17, #19, #20, #30, #31 (verification-gap, pre-verified). The repo has no test harness; Story 1.5 is expected to introduce the first tests.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-1-emit-every-gateway-log-line-through-tracing.md`
  summary: AGENTS.md says "no println! under src/" with nothing enforcing it, and still describes _bmad-output/planning-artifacts as gitignored although it has been committed since b270bd4.
  evidence: Triage #37, and #33's finding that addendum.md is tracked. The fix edits an agent-context file; Story 1.7 owns the AGENTS.md refresh.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-2-move-every-sql-statement-behind-src-store.md`
  summary: Store failure paths have no automated test. Store is a concrete type with no trait or test constructor, so the create store-first order and the once-per-transition readiness WARN can only be proved by stopping a real Postgres.
  evidence: Triage #1, #2, #3. The repo has no #[test]; the smoke client never makes the upsert fail; nothing captures log output. Natural home: Story 1.3 (proving failed-create behaviour) or 1.5 (first tests).
- source_spec: `_bmad-output/implementation-artifacts/spec-1-2-move-every-sql-statement-behind-src-store.md`
  summary: A store write and its cache update are not one unit, so concurrent create/create or create/delete on the same local_ip, or a request cancelled between the two awaits, can leave vpn_routes and the routing table disagreeing until restart.
  evidence: Triage #4, #5. No lock spans upsert_route/delete_route and add_route/remove_route in src/services/gateway.rs. Pre-existing: the old create order had the mirror-image race. replicas: 1 does not prevent it within one pod.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-2-move-every-sql-statement-behind-src-store.md`
  summary: hydrate() reads list_routes and then replaces the routing table wholesale, so a create that commits between the two is dropped from the cache and never reloaded.
  evidence: Triage #6. hydrated=true stops further hydration, and the server accepts gRPC calls while not ready. Pre-existing SELECT-then-load_routes order, moved unchanged.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-2-move-every-sql-statement-behind-src-store.md`
  summary: tunnel_id and remote_endpoint reach the upsert unvalidated, so an empty or over-long value surfaces as Internal carrying raw sqlx error text.
  evidence: Triage #7. Pre-existing; Story 1.5 owns the validation bounds and the INVALID_ARGUMENT mapping.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-2-move-every-sql-statement-behind-src-store.md`
  summary: AGENTS.md still says make check is the only automated verification and states no "SQL only in src/store/" rule, though make check-sqlx and the store boundary now exist.
  evidence: Triage #15. Agent-context file; Story 1.7 owns the AGENTS.md refresh.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-3-persist-before-caching-on-create.md`
  summary: AGENTS.md still says there is no test suite and that make check is the only automated verification, though make test now runs the crate's first unit tests offline.
  evidence: Story 1.3 added `#[cfg(test)] mod tests` in src/services/gateway.rs and the make test target. Agent-context file; Story 1.7 owns the AGENTS.md refresh.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-3-persist-before-caching-on-create.md`
  summary: No unit test covers create's success path, so a regression that drops add_route after a successful upsert passes make test; only the hand-run make smoke against live Postgres catches it.
  evidence: Triage #8. The 1.3 tests use Store::unreachable(), which can only fail. A test with a working store needs either live Postgres or a Store seam the 1.3 spec ruled out. Natural home: Story 1.5, which brings the first broader tests.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-3-persist-before-caching-on-create.md`
  summary: No guidance tells Stories 1.4-1.6 to run make test, so they can move add_route back ahead of the store write in the handlers they rewrite while make check and make check-sqlx stay green.
  evidence: Triage #11 (verification-gap, pre-verified). make check compiles the tests but never runs them; AGENTS.md says there is no test suite; epic-1-context.md names only check and check-sqlx as the enforcement; the 1.7 AC does not mention make test. Fix options: list make test in AGENTS.md (Story 1.7) and in each 1.4-1.6 spec's Verification, or have make check also run cargo test (offline either way).
- source_spec: `_bmad-output/implementation-artifacts/spec-1-4-give-every-route-an-owner-and-scope-writes-to-it.md`
  summary: A database seeded before a schema change (old kind PVC, stale compose volume) hydrates and reports SERVING, then fails every create and delete with INTERNAL because the owner column is missing.
  evidence: Triage #4. hydrate selects only local_ip, tunnel_id, remote_endpoint and readiness runs SELECT 1. Root is the in-place migration 01 plus initdb-on-empty-PVC model; only make cluster-db-reset / db-reset fix it, and nothing points to them at deploy time. Options: verify expected columns at hydrate, or move to forward migrations.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-4-give-every-route-an-owner-and-scope-writes-to-it.md`
  summary: Ownership is verified only by the sequential make smoke; nothing runs OwnedByAnother, the existed-guarded cache removal, or two concurrent foreign-owner creates against Postgres, so a read-then-write regression in upsert_route passes every check.
  evidence: Triage #5 (verification-gap, pre-verified). All unit tests use Store::unreachable(). Cheapest step: two tokio::join!-ed foreign-owner creates in smoke_client asserting exactly one wins; fuller: a #[sqlx::test] harness or a Store seam.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-4-give-every-route-an-owner-and-scope-writes-to-it.md`
  summary: AGENTS.md still says to run make migrate after SQL changes, though migration 01 is edited in place (checksum mismatch on an existing database), and it mentions neither db-reset, cluster-db-reset, nor owner-scoped writes.
  evidence: Triage #6. Agent-context file; Story 1.7 owns the AGENTS.md refresh.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-4-give-every-route-an-owner-and-scope-writes-to-it.md`
  summary: Create and delete on one local_ip are not serialized across their store and cache writes, so concurrent calls can leave the cache holding a deleted route or missing a written one.
  evidence: Triage #13. Pre-existing race. Since 1.4, delete clears the cache only when it removed a row, so a retried delete no longer heals the drift (Triage #12). Fix: a per-local_ip async lock held across store and cache writes.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-5-return-one-failure-taxonomy-from-every-write-path.md`
  summary: A tunnel_id containing CR/LF passes the length-only check, is stored, and is later logged with Display by hydrate's skip warning and route_packet's forwarding line, so it can forge log lines.
  evidence: Triage #2. src/services/gateway.rs logs tunnel_id = %row.tunnel_id and tunnel_id = %route.tunnel_id; both lines predate 1.5, whose 3a decision covered only the first create line and the invalid-local_ip lines. Fix options: reject control characters in validate_tunnel_id (a new bound Story 2.3's CRD must mirror) or log stored strings with ?.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-5-return-one-failure-taxonomy-from-every-write-path.md`
  summary: A delete that commits but loses its connection before the reply returns UNAVAILABLE; the operator's retry gets existed=false, remove_route never runs, and the pod keeps forwarding through the deleted tunnel until restart.
  evidence: Triage #3. delete_vpn_tunnel clears the cache only when existed; hydrate runs once per process. Pre-existing (the same path returned INTERNAL, which Story 3.1 also retries). Same family as the 1.4 create/delete serialization deferral.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-5-return-one-failure-taxonomy-from-every-write-path.md`
  summary: Only waiting for a pool connection is bounded (5s); a query that hangs after acquire (row lock, mid-query partition) has no statement_timeout or socket timeout, so the operator can still hit its own DEADLINE_EXCEEDED.
  evidence: Triage #4. sqlx's acquire deadline (sqlx-core pool/inner.rs:252) covers connecting, not execution. Story 4.4 relies on the gateway answering before the operator deadline. Fix: set statement_timeout via PgConnectOptions options or after_connect, shorter than 3.1's deadline minus the acquire timeout.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-5-return-one-failure-taxonomy-from-every-write-path.md`
  summary: Status messages carry raw Postgres and I/O error text (e.g. column "owner" does not exist, Connection refused) to callers in any namespace.
  evidence: Triage #10. src/store/error.rs formats {source} into every Status message. Pre-existing: "Database persistence failure: {err}" did the same. Fix: generic client message, full text in the server log.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-5-return-one-failure-taxonomy-from-every-write-path.md`
  summary: FAILED_PRECONDITION now depends on upsert_route returning Err(owned_by_another), and no offline test runs that path or checks the cache stays untouched on a conflict.
  evidence: Triage #17 (verification-gap, pre-verified). Reverting the Err to Ok(()) passes all unit tests; only make smoke against live Postgres catches it. Needs a #[sqlx::test] harness or a Store seam, as in the 1.4 deferral.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-5-return-one-failure-taxonomy-from-every-write-path.md`
  summary: AGENTS.md still says there is no test suite and make check is the only automated verification, though make test runs the crate's unit tests.
  evidence: Triage #25. Agent-context file; Story 1.7 owns the AGENTS.md refresh.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-6-read-actual-state-from-durable-storage-with-listroutes.md`
  summary: ListRoutes inherits the unbounded post-acquire query time, so a SELECT waiting on a lock hangs the polling reconciler or surfaces as its own DEADLINE_EXCEEDED instead of a retryable UNAVAILABLE.
  evidence: Triage #3. The handler awaits Store::list_routes with only the 5s acquire bound; same root as the 1.5 statement_timeout deferral, now on the read every reconcile pass depends on.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-6-read-actual-state-from-durable-storage-with-listroutes.md`
  summary: Nothing repeatable proves ListRoutes follows the database when the serving pod's cache differs on the success path; a handler that checked the store and then served the cache would pass make test and make smoke.
  evidence: Triage #12 (verification-gap, pre-verified). Unit tests reach only the database-down path; smoke's cache and DB agree at every ListRoutes check. Covered only by the manual two-gateway check. Needs the DB-backed harness (#[sqlx::test] or a Store seam) deferred in 1.4/1.5.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-7-bring-agents-md-in-line-with-the-code-e1-leaves-behind.md`
  summary: kind/cluster.yaml line 5 ends mid-sentence ("kind publishes through Docker, and."), so the reason the port mapping is bound to 127.0.0.1 is lost.
  evidence: Triage #12. Truncated since it was written in 3bf5cee; the intended clause is not recoverable from history, so only the author can restore it.
- source_spec: `_bmad-output/implementation-artifacts/spec-1-7-bring-agents-md-in-line-with-the-code-e1-leaves-behind.md`
  summary: The kind/cluster.yaml header names smoke_client.rs and health_probe.rs as the host tools that use the 50051 port mapping, but not examples/list_routes.rs (make routes).
  evidence: Triage #13. Pre-existing since Story 1.6 added list_routes.rs; one-word comment fix.
