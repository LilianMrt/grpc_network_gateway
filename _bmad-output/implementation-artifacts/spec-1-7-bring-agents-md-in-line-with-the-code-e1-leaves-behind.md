---
title: 'Bring AGENTS.md in line with the code E1 leaves behind'
type: 'chore'
created: '2026-10-07'
status: 'done'
route: 'oneshot'
review_loop_iteration: 0
baseline_commit: 'f652686623726964112b85f07c47fe10b92e02a4'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** `AGENTS.md` still describes the repository as it was before Epic 1. It says there is no test suite and that `make check` is the only verification. It calls the planning artifacts gitignored, gives no "SQL only in `src/store/`" rule, and says to `make migrate` after SQL changes although migration 01 is edited in place. It also describes reads as pod-local only, although `ListRoutes` now exists, and its kind pin pitfall has no exit. Six deferred-work entries from Stories 1.1–1.5 route these fixes to this story.

**Approach:** Rewrite the affected entries of `AGENTS.md` so every statement matches the code at `f652686`, covering the four epic ACs and each deferred `AGENTS.md` item:
- **Logging:** `tracing` with `tracing-subscriber` is the only logging path; no `println!` remains under `src/` and none may be added; the "Story 1.7 completes…" line is dropped.
- **Store boundary:** AD-2 becomes a rule. All SQL lives in `src/store/`. Handlers call the store first and the cache second. Store errors map to `Status` in one place, `src/store/error.rs`.
- **Checks:** state which check needs Postgres. `make check-sqlx` does; `make check` and `make test` do not. `make test` runs the unit tests.
- **Schema:** two identical copies, edited in place; reset with `make db-reset` or `make cluster-db-reset`.
- **Reads:** `ListRoutes` is the authoritative read; `GetGatewayStatus` and `RoutePacket` serve pod memory. Writes are owner-scoped.
- **Kind pin:** stays at `v1.33.1`; Stories 5.1 (host fix) and 5.3 (pin move) retire it. Sentences that say how earlier text was wrong are removed.
- **Planning artifacts:** described as committed, not gitignored.

Also correct the one factual error the pin pitfall depends on: the `kind/cluster.yaml` comment says the refusal begins at 1.36; it begins at 1.35.

</frozen-after-approval>

## Implementation Notes

- Files: `AGENTS.md` (rewritten inside the `bmad:context` block; verified-against line moved to `f652686`), `kind/cluster.yaml` (comment: 1.36 → 1.35).
- Each statement was checked against the code at `f652686`. The Makefile targets and which of them set `SQLX_OFFLINE` were read from the Makefile. Nothing outside `src/store/` uses sqlx, and `src/` has no `println!` (both checked with grep). The single `From<Error> for Status` is in `src/store/error.rs`. Readiness hydrates once, on the first successful database answer (`src/services/health.rs`). The proto restates the validation bounds. Both schema copies are identical. All 17 planning files are tracked except the gitignored `.memlog.md`. `sqlx-cli` is 0.9.0, and `go` is absent.
- Decisions beyond the four ACs: the opening paragraph no longer says no planning docs are committed. `make migrate` is replaced by `make db-reset`/`make cluster-db-reset` for schema changes. Added entries for the error mapping, ownership, validation bounds (changed in lockstep with proto and CRD), and the `ListRoutes`-versus-cache split. `make test` is listed as an offline check (deferred items from 1.3 and 1.5). The `println!` rule names `grep -rn println src/` as its check, because nothing enforces it (deferred item from 1.1).
- The second "how the earlier text was wrong" sentence ("not 1.36 as this file previously said") was also removed, matching the logging AC's intent.
- `make check` passes.


## Review Triage Log

Pass 1 (oneshot). Source: blind-hunter.

| # | Finding | Verdict | Evidence | Route |
|---|---------|---------|----------|-------|
| 1 | The new SQL-change recipe dropped `make migrate`, and `check-sqlx` needs a *migrated* database | medium | Real: `compose.yaml` mounts no init scripts, so a fresh volume has no `vpn_routes` and `prepare` fails. | patch |
| 2 | `check-sqlx` fails on correct-but-uncommitted `.sqlx/` and leaves files behind; "never touch `.sqlx/`" is misleading for check/test | low | Real: the Makefile tests `git status --porcelain`; offline builds read `.sqlx/`. | patch |
| 3 | No unit test reaches real SQL, so smoke is required after store changes; unit tests also live in validation.rs and store/error.rs | medium | Real: all handler tests use `Store::unreachable()`; the test counts are 6 in validation.rs and 3 in error.rs. | patch |
| 4 | Delete idempotency omitted; `FAILED_PRECONDITION` is create-only | low | Real: `proto/gateway.proto` service comment. | patch |
| 5 | Owner format reads as enforced | low | Real: `check_owner` rejects only empty; the column is `TEXT`. | patch |
| 6 | The change-together list is incomplete; `POOL_ACQUIRE_TIMEOUT` is not mentioned | low | The list is real: the `validation.rs` doc names five places. The pool timeout already has a point-of-use "change both together" in `main.rs` and the proto, so it is not repeated. | patch (list) / reject (timeout) |
| 7 | "Context in fields, not the message" conflicts with the quoted hydration line | low | Real: `health.rs` interpolates the count, and the demo relies on the text. | patch |
| 8 | The liveness/readiness split is unrecorded; `health.rs` is missing from the map | low | Real omission; it is a defining convention ("liveness never depends on the database"). | patch |
| 9 | `db-reset` leaves a running `make run` with a stale cache | low | Real: hydrate runs once; the Makefile comment says to restart. | patch |
| 10 | Policy omits the per-story `docs:` commit on the feature branch | low | Real: 546f97d and 1d3f07e follow that pattern. | patch |
| 11 | The implementation-artifacts listing is incomplete | low | Real: it omitted `sprint-status.yaml` and `epic-1-context.md`. | patch |
| 12 | `kind/cluster.yaml` says "drop" the pin rather than move it; line 5 is truncated; KEP not cited | low | "Drop" contradicts AD-11 and Story 5.3, so it is patched. The KEP is cited in AGENTS.md (rejected). The line 5 truncation predates this story (3bf5cee) and its intent is unrecoverable. | patch / reject / defer |
| 13 | The kind header omits `list_routes.rs` | low | Real, but pre-existing since 1.6. | defer |
| 14 | The SQL rule has no grep check, unlike the println rule | low | Real; a one-line addition. | patch |

After the patches, `make check` passes, and both `grep -rn println src/` and `grep -rn sqlx src/ --exclude-dir=store` are empty.
