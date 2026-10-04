---
title: 'Emit every gateway log line through tracing'
type: 'refactor'
created: '2026-10-04'
status: 'done'
route: 'dispatch'
review_loop_iteration: 1
baseline_commit: 'e044c6e398267e33b6178ed185c0e1d5df179ebd'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The gateway logs only with `println!`. `tracing` is declared in `Cargo.toml`, but no subscriber is installed and nothing calls it, so lines carry no structured context, and error returns are not logged at all.

**Approach:** Add `tracing-subscriber` and initialise it before anything else runs. Then replace every `println!` under `src/` with a leveled `tracing` macro that carries fields: `local_ip` on create, delete and hydration lines, and `code` on every handler error return (spine AD-9, FR-26).

## Boundaries & Constraints

**Always:**
- Output is human-readable `fmt` text on stdout.
  - Default filter is `info`; `RUST_LOG` overrides it.
  - ANSI colour only when stdout is a terminal, so `kubectl logs` stays clean.
- The hydration line's message keeps the literal text `hydrated N route(s) from the database`. The FR-10 demo (addendum §B) quotes it.
- Fields are fields, never interpolated into the message: `local_ip`, `tunnel_id`, `remote_endpoint`, `existed`, `dest_ip`, `code`.
  - Use `%` for Display values.
  - Use `code = ?status.code()` so the code renders as e.g. `code=Internal`.
- Each handler error is logged once, at the point where its `Status` is built, using the same `Status` value that is returned. Invalid input logs at `warn`, a database failure at `error`.
- Initialisation lives in the library. `main.rs` only calls it.

**Never:**
- Do not change any gRPC response, status code, or status message. The taxonomy is Story 1.5's.
- Do not touch SQL, so `.sqlx/` stays unchanged. Do not move SQL; that is Story 1.2.
- Do not convert `examples/`: their `println!` is CLI output for a person, not service logging.
- No JSON output, no OpenTelemetry, no spans or `#[instrument]`.
- Do not rewrite `AGENTS.md` beyond its logging bullet. The full refresh is Story 1.7.

</frozen-after-approval>

## Code Map

- `Cargo.toml` -- `tracing = "0.1"` is already present. Add `tracing-subscriber = { version = "0.3", features = ["env-filter"] }`.
- `Cargo.lock` -- must be committed. The `Dockerfile` builds with `--locked`.
- `src/lib.rs` -- module list. Register the new `logging` module.
- `src/main.rs:22` -- `dotenvy::dotenv()` comes first. Logging init goes right after it, so a `.env`-supplied `RUST_LOG` applies.
- `src/main.rs:33` -- `println!("gRPC Control Plane listening on {}", addr)`.
- `src/services/gateway.rs:92-110` -- `hydrate()`. It silently skips rows whose `local_ip` fails to parse. That skip is the only per-IP hydration event.
- `src/services/gateway.rs:131` -- create "Received request" line, plus three error sites:
  - the `invalid_argument` at 133-139;
  - the `internal` at 159-161;
  - the delete `invalid_argument` at 177-183 and `internal` at 188.
- `src/services/gateway.rs:198` -- delete outcome line (`existed`).
- `src/services/gateway.rs:227-236` -- per-packet forward and drop lines. These are a demo surface, so they stay at `info`.
- `src/services/health.rs:63` -- the hydrated line.
- `src/services/health.rs:77` -- readiness transition line, printed only on change. Keep that behaviour.
- `AGENTS.md` -- the "Logging is `println!` throughout today" bullet becomes false after this change.
- `.env.example` -- holds only `DATABASE_URL`. This is where a local `RUST_LOG` goes.
- `tracing-subscriber` 0.3.23 `EnvFilter::builder()`:
  - `.with_default_directive(LevelFilter::INFO.into()).from_env_lossy()` treats an unset or empty `RUST_LOG` as `info`.
  - It skips an invalid directive and prints `ignoring ...` on stderr.
  - `try_from_default_env()` parses an empty `RUST_LOG` to a filter that drops everything.

## Tasks & Acceptance

**Execution:**
- [x] `Cargo.toml`, `Cargo.lock` -- add `tracing-subscriber` with `env-filter`, and let cargo update the lock.
- [x] `src/logging.rs` (new), `src/lib.rs` -- add `pub fn init()`. It installs the `fmt` subscriber with `EnvFilter::builder().with_default_directive(LevelFilter::INFO.into()).from_env_lossy()` and sets `with_ansi(stdout().is_terminal())`. Do not use `try_from_default_env()`: an empty `RUST_LOG` would silence every line.
- [x] `src/main.rs` -- call `logging::init()` after `dotenv()`. Turn the listening line into `info!(%addr, ...)`.
- [x] `src/services/gateway.rs` -- create line with `local_ip` and `tunnel_id` fields. Delete outcome line with `local_ip` and `existed`. In `hydrate()`, a `warn!` with `local_ip` for each skipped unparseable row. Route-packet lines with fields. A `warn!` or `error!` carrying `local_ip`, `code` and `error` at each of the four error sites.
- [x] `src/services/health.rs` -- hydrated line as `info!(count, "hydrated {} route(s) from the database", count)`, with a one-line comment saying the count stays in the message because the FR-10 demo quotes that literal text. Transition line as `info!(%status, ...)`.
- [x] `.env.example` -- add a commented `# RUST_LOG=info` line, with a short note that it overrides the default `info` filter. Keep it commented: a live `RUST_LOG=` with no value is a real setting.
- [x] `AGENTS.md` -- reword only the logging bullet: `tracing` is now the path, `println!` is gone from `src/`, and Story 1.7 completes the refresh.

**Acceptance Criteria:**
- Given the built gateway, when it starts, then `tracing` lines (timestamp, level, target, message, fields) appear on stdout, beginning with the listening line.
- Given the tree, when `grep -rn 'println!' src/` runs, then it prints nothing.
- Given `make smoke` against a running gateway, when it creates and deletes, then the create and delete lines show `local_ip=10.…` as a field.
- Given Postgres is stopped after startup, when a create is sent, then an `ERROR` line shows `code=Internal` and the client receives the same status as before this change.
- Given Postgres is up, when the gateway starts, then a line containing `hydrated N route(s) from the database` appears once.
- Given `RUST_LOG=warn`, when the gateway starts, then the `info` lines are suppressed.
- Given `RUST_LOG=` (empty) or an invalid directive, when the gateway starts, then the `info` lines still appear. For an invalid directive, stderr names the ignored directive.

## Implementation Notes

- `tracing-subscriber` resolved to 0.3.23. `Cargo.lock` only added entries: `tracing-subscriber`, `tracing-log`, `matchers`, `sharded-slab`, `thread_local`, `nu-ansi-term`, `lazy_static`, `valuable`.
- Each error site builds the `Status` first, logs it with `code = ?status.code()`, then returns that same value. Status codes and messages are unchanged.
- The delete outcome line is `info!(%local_ip, existed, "tunnel delete handled")`. The interpolated `status_message` still goes in the response, unchanged.
- Verified against a live gateway:
  - `make smoke` passed.
  - With the database down, a create logged `ERROR … code=Internal`.
  - An invalid IP on create or delete logged `WARN … code=InvalidArgument`.
  - With `RUST_LOG=warn`, the `info` lines were suppressed.
  - Output redirected to a file had no ANSI codes.
- Never exercised: the `warn` for an unparseable hydrated row, because no bad row existed. Also never run: a container build with `--locked`, and `kubectl logs`.
- Seen, not fixed (Story 1.3): after a create fails at the database, `RoutePacket` still forwards, because memory is written before the database.
- With a subscriber installed, `sqlx` and `tonic` lines now appear at `info`/`warn`. Examples: a sqlx slow-statement warning on the first query, and a pool line when Postgres drops.

- Loop 1, patched in place at the user's choice. The full revert was blocked by the permission check, and the user chose an in-place patch.
  - `src/logging.rs` now uses `EnvFilter::builder().with_default_directive(INFO).from_env_lossy()`.
  - `health.rs` has the FR-10 comment.
  - `.env.example` has a commented `RUST_LOG` line, plus the trailing newline it was missing.
  - Observed: unset, empty and invalid `RUST_LOG` all print the `info` listening line. The invalid case prints ``ignoring `grpc_network_gateway=debgu`: …`` on stderr.

## Spec Change Log

- **Loop 1 (review pass 1).**
  - **Triggers:** triage #1 and #2 (filter construction), #13 (unexplained interpolation of `count`), #15 (`RUST_LOG` undocumented).
  - **Amended:**
    - The `src/logging.rs` task now uses `EnvFilter::builder().with_default_directive(INFO).from_env_lossy()`.
    - The `health.rs` task requires the FR-10 comment.
    - New `.env.example` task.
    - Code Map and ACs extended to match.
  - **Known-bad state avoided:**
    - `RUST_LOG=` silenced every line.
    - An invalid `RUST_LOG` was dropped with no message.
    - A later "fields, not message" cleanup could break the literal the FR-10 demo quotes.
  - **KEEP** (worked well; must survive re-derivation):
    - `tracing-subscriber = { version = "0.3", features = ["env-filter"] }`, with `Cargo.lock` only gaining entries.
    - `logging::init()` called in `main.rs` right after `dotenv()`, with its comment.
    - `with_ansi(stdout().is_terminal())`.
    - At each of the four handler error sites: build the `Status` first, log with `local_ip`, `code = ?status.code()` and `error = %err` (`warn` for invalid input, `error` for the DB), then return that same `Status`.
    - The `hydrate()` `match` with a `warn!` (`local_ip`, `tunnel_id`, `error`) for unparseable rows.
    - Delete outcome `info!(%local_ip, existed, "tunnel delete handled")`.
    - Forward and drop lines with `dest_ip`, `tunnel_id`, `remote_endpoint`.
    - Readiness transition `info!(%status, "readiness status changed")`.
    - The reworded `AGENTS.md` logging bullet.

## Review Triage Log

Pass 1 (loop 0). Sources: B = blind-hunter, E = edge-case-hunter, V = verification-gap.

| # | Source | Finding | Verdict | Evidence | Route |
|---|--------|---------|---------|----------|-------|
| 1 | E | An empty `RUST_LOG` disables every line | medium | Reproduced: with `RUST_LOG=` the gateway printed nothing, while unset printed the listening line. `try_from_default_env` parses `""` to an empty filter with no default. | bad_spec |
| 2 | B, E | An invalid `RUST_LOG` silently falls back to `info` | low | Reproduced with `grpc_network_gateway=debgu`: `info` output appeared and nothing on stderr said the directive was dropped. Same root cause as #1, the filter construction. | bad_spec (grouped with #1) |
| 3 | B, E | `logging::init()` panics if called twice | false | It has one caller (`main.rs`), its doc says "call once", and there is no `tests/` directory. Failing loudly on an unreachable misuse is correct. | reject |
| 4 | E | An explicit `with_ansi` overrides `NO_COLOR` on a terminal | low | Real by `fmt` semantics, but it needs both a TTY and `NO_COLOR`, and the fix adds a condition. | reject |
| 5 | B, E | Raw client strings are logged with Display, which allows newline injection | low | Pre-existing: `tunnel_id` was interpolated by `println!`, and stored `tunnel_id` and `remote_endpoint` were printed on forward. This diff extends it to the raw `local_ip`. Exploiting it needs a hostile client on the cluster network. Switching to `?` contradicts the frozen `%` rule. | defer |
| 6 | E | Fields as large as the gRPC limit are logged untruncated | low | Unlikely, and truncation adds code. The `Status` message already echoes the same value. | reject |
| 7 | E | A failed create leaves the route live in memory | high | Pre-existing: `add_route` runs before the INSERT. Story 1.3 owns the fix. | defer |
| 8 | B, E | A successful create has no outcome line | low | Pre-existing: there was no success `println!` either. | defer |
| 9 | B, E | `MALFORMED_HEADER` drops are not logged | low | Pre-existing: that early return never printed. | defer |
| 10 | B, V, E | Readiness hydrate and ping errors are discarded with no cause logged | medium | Pre-existing: `Err(_) => false` and `.is_ok()` are unchanged by the diff (`health.rs:60,68`). | defer |
| 11 | B, E | The listening line is emitted before the bind | low | Pre-existing order in `main.rs`. | defer |
| 12 | B, E | `main`'s `?` errors bypass `tracing` and go to stderr | low | Pre-existing: they go through `Termination`. This includes the module-doc claim. | defer |
| 13 | B, V, E | The hydrated line repeats `count` in the message with no explanation, against the new `AGENTS.md` rule | low | Real: the next cleanup that follows "fields, not message" breaks the literal FR-10 quotes. The spec mandated the interpolation but not the explanation. | bad_spec |
| 14 | B | The default `info` filter is not crate-scoped and lets in third-party noise | false | The third-party output observed was a sqlx slow-statement `WARN`, which a scoped `warn,...` filter keeps too, and one pool line on outage, which is signal. The claimed noise does not appear. | reject |
| 15 | B | `RUST_LOG` is not documented in `.env.example` or `k8s/` | low | Real: `.env.example` lists only `DATABASE_URL`. The spec added the knob without saying where it is documented. | bad_spec |
| 16 | B | The readiness line omits the service name | false | `health.rs` always sets `OVERALL` and `service_name` to the same status in the same tick, so there is nothing to tell apart. | reject |
| 17 | B, V | The unparseable-row `warn` arm has never run, and the container build and `kubectl logs` are unverified | gap | V, pre-verified: no test references `hydrate`. V also ran `cargo check --locked --offline`, which passes, so the lockfile half is settled. | defer |
| 18 | B | No lint enforces "no `println!` in `src/`" | false | The proposed `clippy::print_stdout` deny is never evaluated: `make check` runs `cargo check`, not clippy, and there is no CI. | reject |
| 19 | V | Subscriber defaults (level, ANSI) have no regression test | gap | V, pre-verified: no test harness exists. | defer |
| 20 | V | The four handler error paths have no regression test | gap | V, pre-verified. Story 1.5 owns the taxonomy and its first tests. | defer |

Pass 2 (loop 1). Same sources.

| # | Source | Finding | Verdict | Evidence | Route |
|---|--------|---------|---------|----------|-------|
| 21 | B, E | Raw client strings are logged with Display, which allows CR/LF injection | low | carried: #5. The code still reads as described. | defer |
| 22 | B, E | `logging::init()` panics if called twice | false | carried: #3. | reject |
| 23 | E | `with_ansi` overrides `NO_COLOR` | low | carried: #4. | reject |
| 24 | B, E | The listening line is logged before the bind | low | carried: #11. | defer |
| 25 | B, E | `main`'s `?` errors bypass `tracing` | low | carried: #12. | defer |
| 26 | B, E | Readiness hydrate and ping errors are discarded; the NotServing transition is only `info`, so `RUST_LOG=warn` hides an outage | medium | carried: #10. The level aspect has the same root cause, the readiness path's logging is unchanged. | defer |
| 27 | B, E | A failed create leaves the route live in memory; the new "create failed" line reads as if it were absent | high | carried: #7. The new message matches the `Internal` the client receives. The memory state is Story 1.3's defect. | defer |
| 28 | B, E | Create and delete logging is asymmetric: no success line for create | low | carried: #8. | defer |
| 29 | E | `MALFORMED_HEADER` drops are not logged | low | carried: #9. | defer |
| 30 | B, V | The filter, the error-path `Status`, and the unparseable-row arm have no regression test | gap | carried: #17, #19, #20. | defer |
| 31 | V | The FR-10 hydrated text is protected only by a source comment, with no test | gap | V, pre-verified: no test harness exists. | defer |
| 32 | E | A bare misspelled level (`RUST_LOG=warning`) silences every line with no stderr | low | Reproduced: `warning` and `verbose` print nothing on either stream. `EnvFilter` parses a bare word as a target directive, the standard `RUST_LOG` grammar, so it is not an invalid directive. A guard needs custom validation. | reject |
| 33 | B | The FR-10 comment cites a demo that lives only in gitignored docs | false | `addendum.md` is tracked (`git ls-files`, committed in b270bd4). `.gitignore` does not exclude `_bmad-output/`. | reject |
| 34 | B | The `hydrated N` count hides skipped rows | low | Each skipped row already gets its own `warn` line. Bad rows need a non-IP `local_ip`, which create already rejects. A skipped counter adds code. | reject |
| 35 | B | `RUST_LOG` cannot be set through the k8s ConfigMap | low | Developer-only. `kubectl set env deployment/gateway RUST_LOG=…` works today, and `AGENTS.md` documents the variable. | reject |
| 36 | B | The `.env.example` example `RUST_LOG=grpc_network_gateway=debug` changes nothing | low | Real: the crate has no `debug!` call sites, so the example output equals the default. A direct text correction fixes it. | patch |
| 37 | B | The `AGENTS.md` bullet is unenforced ("no `println!`") and cites Story 1.7 | low | The enforcement half is carried from #18 (false). The fix edits an agent-context file. | defer |
| 38 | E | Per-packet `info` lines can flood at high packet rates | low | `RoutePacket` is a demo surface with no real traffic. The spec kept it at `info` on purpose, matching the old `println!`. | reject |

## Verification

**Commands:**
- `make check` -- expected: compiles with no new warnings and no `.sqlx/` diff.
- `grep -rn 'println!' src/` -- expected: no output.
- `git status --short .sqlx` -- expected: empty.

**Manual checks:**
- `make db-up && make run`, then `make smoke` in a second shell. The output shows the listening, hydrated, create and delete lines with fields.
- `make db-down`, then send a create. The output shows an `ERROR … code=Internal` line.
