---
title: 'Return one failure taxonomy from every write path'
type: 'feature'
created: '2026-10-07'
status: 'done'
baseline_commit: '745612ecd499367577db7ed45ae90d64e49f0d3d'
route: 'dispatch'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** An unreachable database returns `INTERNAL` from both writes, a Postgres constraint violation (e.g. a 300-char `tunnel_id`, `22001`) also returns `INTERNAL` and is retried forever, and the pool waits sqlx's default 30s before failing (FR-33, AD-3, AD-15).

**Approach:** Classify every `sqlx::Error` once, at the store boundary, into `Unavailable` / `InvalidArgument` / `OwnedByAnother` / `Internal`, with one `From<store::Error> for Status`. Validate `tunnel_id` and `remote_endpoint` against recorded bounds before any write. Set the pool acquire timeout explicitly in `src/main.rs`.

## Boundaries & Constraints

**Always:**
- Classification reads the `sqlx::Error` variant and the SQLSTATE `code()`, never the message text.
- `Unavailable`: `PoolTimedOut`, `PoolClosed`, `Io`, `Tls`, `WorkerCrashed`, and SQLSTATE classes `08`, `53`, `57P01`–`57P03`. `InvalidArgument`: SQLSTATE classes `22` and `23`. Everything else, including class `42` (a stale schema), is `Internal`.
- The ownership conflict leaves the store as `Error::OwnedByAnother` mapped to `FAILED_PRECONDITION`; `UpsertOutcome` goes away. Conflict message unchanged and still never names the holder.
- Validation order: `local_ip`, `owner`, `tunnel_id`, `remote_endpoint`; all before any store call. Lengths count characters (Postgres `VARCHAR(n)` semantics), not bytes.
- Bounds live as `pub const`s next to the validators and are restated in `proto/gateway.proto` comments, the cross-language record Story 2.3 mirrors.
- Acquire timeout: 5s, passed from `src/main.rs` into `Store::connect_lazy`. Story 3.1's default deadline must exceed it; say so beside the constant.
- Store first, cache second, unchanged. `make check` and `make test` stay offline. No SQL text changes, so `.sqlx/` stays byte-identical.

**Never:**
- No `ListRoutes` (1.6), no AGENTS.md edit (1.7), no owner format/length check (1.4 decision), no schema change.
- No mapping logic in handlers beyond logging the resulting `Status`.

**Decisions (2026-10-07, user):**
- `remote_endpoint` is strict `host:port`, at most 255 chars. Split on the last `:`. Port: ASCII digits only, `1`–`65535`. Host: either a dotted IPv4 address accepted by `Ipv4Addr::from_str` (so no leading zeros), or an RFC 1123 hostname — dot-separated labels of 1–63 `[A-Za-z0-9-]`, no leading/trailing `-`, at most 253 chars, no trailing dot. A host made only of digits and dots must be IPv4 (rejects `10.0.0.999`). No IPv6.
- `tunnel_id` is 1–255 chars; empty is rejected.
- CR/LF log injection is in scope: client strings not yet validated are logged with `?` (Debug) — the first create line and the invalid-`local_ip` lines. Validated values keep `%`.
- Spec kept above 1600 tokens.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| DB down, create/delete | pool cannot connect | `UNAVAILABLE` within ~5s; cache unchanged | logged with `local_ip`, `code` |
| `tunnel_id` 256 chars | create | `INVALID_ARGUMENT`; no store call | logged |
| `tunnel_id` 255 multibyte chars | create | accepted by validation | N/A |
| Empty `tunnel_id` | create | `INVALID_ARGUMENT`; no store call | logged |
| Good `remote_endpoint` | `"203.0.113.7:51820"`, `"vpn-1.example.com:443"`, `"gw:1"` | accepted | N/A |
| Bad `remote_endpoint` | `"no-port"`, `"h:0"`, `"h:70000"`, `"h:+80"`, `":51820"`, `"a$b:80"`, `"-a.com:80"`, `"10.0.0.999:80"`, `"[::1]:80"`, `"a.com.:80"` | `INVALID_ARGUMENT`; no store call | logged |
| Constraint violation | SQLSTATE `22001` / `23502` from store | `INVALID_ARGUMENT` | permanent |
| Foreign owner | row held by another owner | `FAILED_PRECONDITION` | unchanged from 1.4 |
| Unexpected | SQLSTATE `42703`, `Protocol`, `Decode` | `INTERNAL` | logged |

</frozen-after-approval>

## Code Map

- `src/store/mod.rs:15-17` -- `pub type Error = sqlx::Error` is the seam: replace with `pub struct Error { kind: ErrorKind, source: Option<sqlx::Error> }` (or an enum) plus `pub fn kind()`, `Display`, `From<sqlx::Error>` (the classifier), and `From<Error> for tonic::Status`. Put it in a new `src/store/error.rs`, re-exported. Status message: `"database unavailable: {source}"` / `"rejected by the database: {source}"` / `"unexpected database failure: {source}"`.
- `src/store/mod.rs:28-35,87-113` -- delete `UpsertOutcome`; `upsert_route` returns `Result<(), Error>`, `rows_affected() == 0` → `Error::owned_by_another(local_ip)`.
- `src/store/mod.rs:47-50` -- `connect_lazy(url, acquire_timeout: Duration)`. `unreachable()` (test) keeps its 200ms.
- `src/main.rs:164` -- `const POOL_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);` with the 3.1 note.
- `src/services/validation.rs` (new, `pub mod` in `src/services/mod.rs`) -- `MAX_TUNNEL_ID_CHARS = 255`, `MAX_REMOTE_ENDPOINT_CHARS = 255` (cite both schema files), `validate_tunnel_id`, `validate_remote_endpoint`, returning `Result<(), Status>` with `INVALID_ARGUMENT`. Move `check_owner` here.
- `src/services/gateway.rs:84-104,162-175` -- raw `payload.*` fields in the first create log line and both invalid-`local_ip` lines switch from `%` to `?`.
- `src/services/gateway.rs:107-140,178-192` -- `map_err(Status::from)` plus one `error!`/`warn!` with `code`; drop the handler-side conflict branch.
- `src/services/gateway.rs:343-390` -- `assert_internal` → `assert_unavailable`; three DB-down tests expect `Code::Unavailable`.
- `src/services/health.rs` -- only `%err` on `store::Error`; needs `Display`, nothing else.
- `proto/gateway.proto` -- field comments for bounds; a service comment listing the four codes.
- `examples/smoke_client.rs` -- one step: create with `remote_endpoint: "no-port"` expects `InvalidArgument`, before the deletes.

## Tasks & Acceptance

**Execution:**
- [x] `src/store/error.rs`, `src/store/mod.rs` -- error type, classifier, `Status` mapping, `OwnedByAnother`, acquire-timeout parameter.
- [x] `src/main.rs` -- explicit acquire timeout constant.
- [x] `src/services/validation.rs`, `src/services/mod.rs` -- bounds and validators.
- [x] `src/services/gateway.rs` -- use validators and `Status::from`; update tests.
- [x] Tests -- classifier table test using a test-only `DatabaseError` impl carrying a SQLSTATE (`22001`, `23502`, `08006`, `57P03`, `42703`) plus `PoolTimedOut`/`Protocol`; validator boundary tests (255/256 chars, multibyte, each bad endpoint); handler tests: overlong `tunnel_id` and bad endpoint return `InvalidArgument` against the unreachable store with the cache unchanged.
- [x] `proto/gateway.proto`, `examples/smoke_client.rs` -- comments; invalid-endpoint smoke step.

**Acceptance Criteria:**
- Given Postgres stopped, when `make check` and `make test` run, then both pass and `git status --porcelain .sqlx` is empty.
- Given `make run` with Postgres stopped, when a create is sent, then it returns `UNAVAILABLE` in about 5s, not 30s.
- Given `make run` against a live database, when `make smoke` runs, then it passes, including the invalid-endpoint step.
- Given the source, when grepped, then `Status::internal`/`unavailable` for store failures appears only in `src/store/error.rs`.

## Implementation Notes

## Spec Change Log

## Review Triage Log

Pass 1 (loop 0). Sources: B = blind-hunter, E = edge-case-hunter, V = verification-gap.

| # | Source | Finding | Verdict | Evidence | Route |
|---|--------|---------|---------|----------|-------|
| 1 | B, E, V | `owner` is logged with `%` in the new `log_failed_write` and the delete `info!`, though only emptiness is checked, so CR/LF forges log lines | medium | Real: `gateway.rs:61,64,199`. The 3a decision logs unvalidated client strings with `?`; `owner` is never content-validated. Direct `%`→`?` change. | patch |
| 2 | B, E | `tunnel_id` with CR/LF passes the length check, is stored, then logged with `%` by `hydrate` and `route_packet` | medium | Real, but those two lines predate this story and the 3a decision named the lines in scope. Fixing it means a new validation rule (CRD-mirrored) or reformatting pre-existing lines. | defer |
| 3 | B | A delete that commits and then loses its connection returns `UNAVAILABLE`; the retry gets `existed=false` and never clears the cache | medium | Real and pre-existing (it returned `INTERNAL` before, which 3.1 also retries). Same family as 1.4 #12/#13. | defer |
| 4 | B, E | The 5s bound covers acquire only; a query that hangs after acquire has no `statement_timeout`, so the operator can still see `DEADLINE_EXCEEDED`, which the `main.rs` comment over-promises | medium | Real: sqlx's acquire deadline covers connecting (`pool/inner.rs:252`), not execution. The spec's AC is acquire only; the comment overclaims. The timeout itself is out of scope. | patch (comment) + defer (timeout) |
| 5 | B, E | `40001`/`40P01`/`55P03`/`25006` classified `INTERNAL`, not `UNAVAILABLE` | low | Real labels, but Story 3.1 retries `INTERNAL` too, so the caller behaves the same; single autocommit statements rarely hit them. The frozen list is explicit. | reject |
| 6 | B, E | `Tls` is always `Unavailable`, even for a permanent certificate error | low | It is in the frozen `Unavailable` list, and 3.1 retries `INTERNAL` too, so reclassifying would not stop the retries. | reject |
| 7 | B, V | Classifier test misses `Tls`, `WorkerCrashed`, `57P02`, a `None` SQLSTATE, and a near-miss like `57P04` | medium | V pre-verified: dropping any of those arms passes all 16 tests. Adds only table rows. | patch |
| 8 | B | `Error` can represent a `kind` that contradicts its `repr` | low | Developer-only; no caller can build it (`owned_by_another` and `From<sqlx::Error>` are the only constructors). Fix is a restructure. | reject |
| 9 | B | Failed-write logs carry only `status.message()`, not the SQLSTATE or kind as fields | low | The message already includes the Postgres text; the fix adds fields to a shared logger for marginal gain. | reject |
| 10 | B | Raw database error text goes back to clients | low | Pre-existing: `"Database persistence failure: {err}"` did the same before this story. | defer |
| 11 | B | The proto says every write RPC can fail `FAILED_PRECONDITION`, but a foreign-owner delete succeeds with `existed=false` | low | Real: `gateway.proto:4-11`. Direct comment correction. | patch |
| 12 | B | Bounds live in five places, but `validation.rs` says "change all three together"; the `main.rs` 5s constant doesn't mention the proto | low | Real: the two schema files, the consts, the proto, and the CRD. Direct comment correction. | patch |
| 13 | B, E | Ports with leading zeros (`h:080`) are accepted, and the proto doesn't say so | low | The code matches the frozen rule (ASCII digits, 1–65535). The harm is Story 2.3 mirroring it wrongly, so state it in the proto comment. | patch |
| 14 | B | The smoke step asserts the code but not that nothing was stored | low | Covered offline by `create_with_bad_endpoint_is_rejected_before_any_write`, which asserts the cache is unchanged. | reject |
| 15 | V | `connect_lazy`'s acquire timeout is never exercised: `Store::unreachable()` builds its own pool | medium | V pre-verified: ignoring the parameter passes every test. Routing `unreachable()` through `connect_lazy` is a one-line change. | patch |
| 16 | V, B | Documented validation order is not pinned by a test | low | V pre-verified: reordering the `and_then` chain passes. Adds one test. | patch |
| 17 | V | `FAILED_PRECONDITION` now depends on `upsert_route` returning `Err`, and no offline test reaches it | medium | V pre-verified. Same harness gap as 1.4 #5; `make smoke` is the guard. | defer |
| 18 | E | A non-ASCII host over 253 bytes but at most 253 chars is reported as too long instead of invalid characters | low | Real: the byte-length check runs before the charset check, so the comment "every accepted character is ASCII" doesn't hold there yet. Still rejected; moving the check is direct. | patch |
| 19 | E | `23505` on the serial `id` would be classed permanent | low | Needs hand-inserted rows that push the sequence behind; unlikely. | reject |
| 20 | E | `28P01`/`3D000` return `INTERNAL`, though the proto says "cannot be reached" is `UNAVAILABLE` | false | The database was reached; a misconfigured credential or database name is unexpected, which is `INTERNAL`. | reject |
| 21 | E | `0.0.0.0`, broadcast and multicast hosts are accepted | low | The frozen rule accepts any `Ipv4Addr::from_str`; tightening it changes a CRD-mirrored bound. Unlikely in practice. | reject |
| 22 | E | An invalid `DATABASE_URL` at startup reads "unexpected database failure: …" | low | The sqlx configuration text is still in the message. | reject |
| 23 | E | Removing `UpsertOutcome` and the `Error` alias breaks the public API | false | No user outside the crate; the spec requires the removal. | reject |
| 24 | E | Comment says "a value past here fits its column", but a NUL still reaches Postgres (`22021`) | low | The comment calls the database checks a backstop, and `22021` maps to `INVALID_ARGUMENT` as intended. | reject |
| 25 | V | AGENTS.md still says there is no test suite | low | Agent-context file; Story 1.7 owns it. | defer |

## Verification

**Commands:**
- `make db-down; make check; make test` -- expected: clean, all tests pass, `.sqlx/` untouched.
- `make db-up && make check-sqlx` -- expected: passes with no `.sqlx/` change.
- `make run` then `make smoke` -- expected: OK line.
- `grep -rn sqlx src/ --exclude-dir=store; grep -rn println src/` -- expected: no output.

**Manual checks:**
- With `make run` and Postgres stopped, `time make smoke` fails with `Unavailable` after ~5s.
