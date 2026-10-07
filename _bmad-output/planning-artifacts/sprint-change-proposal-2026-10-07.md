# Sprint Change Proposal: Epic 1 follow-up before Epic 2

- **Date:** 2026-10-07
- **Author:** Lilian (with Claude, via `bmad-correct-course`)
- **Mode:** batch
- **Status:** approved by Lilian 2026-10-07; §4.1–§4.5 applied
- **Scope:** Moderate. Three stories added to Epic 1 and planning artifacts amended. No rollback, no MVP change.

## 1. Issue summary

The Epic 1 retrospective (`_bmad-output/implementation-artifacts/epic-1-retro-2026-10-07.md`) accepted the epic with open items. Three of its findings change contracts that Epic 2 and Epic 3 build on, so Lilian decided on 2026-10-07 to settle all three **before Epic 2**.

1. **F-5: ownership is invisible on read.** `ListRoutes` omits `owner` (AD-12), and "already correct" compares `tunnel_id` and `remote_endpoint` only. A second `VpnTunnel` whose spec matches another resource's row reports `Ready=True/Converged` without owning that row, and the conflict never surfaces. This was a **known limit accepted on 2026-10-04** (`epics.md:698`). Lilian reverses that acceptance. AD-13 already wants a conflict to be "a permanent `Ready=False` reason", and putting `owner` on the wire lets Story 3.4 raise `OwnedByAnother` on read without a write.
2. **F-9: identifiers reach Postgres unchecked for content.** `tunnel_id` and `owner` are checked for length or emptiness only. A NUL byte was observed live returning `INVALID_ARGUMENT` with raw Postgres text, and CR/LF in a stored `tunnel_id` can forge log lines (open deferral 1.5 #2). Any new bound must be mirrored by Story 2.3's CRD markers (AD-15), so it has to land before Epic 2.
3. **F-20: the store's SQL is never tested automatically.** A database-backed harness was deferred five times (1.2, 1.3, 1.4, 1.5, 1.6). The owner-guarded upsert, the owner-scoped delete, concurrency and `ListRoutes` against a cache that disagrees with the database are checked only by the sequential `make smoke` and by hand.

Evidence: the retro's Findings (F-5, F-9, F-20) and its Behavior verification table, which records the NUL leak and the 30/30 concurrency run.

## 2. Impact analysis

### Epic impact

| Epic | Impact |
|---|---|
| **E1** | Reopened: three stories (1.8, 1.9, 1.10) added. `epic-1` goes back to `in-progress`; `epic-1-retrospective` stays `done`. |
| **E2** | Story 2.3 mirrors the new `tunnelID` pattern. The dependency note names Story 1.8 alongside 1.5. |
| **E3** | Story 3.2's "already correct" includes `owner`. Story 3.4 raises `OwnedByAnother` from a read. The 2026-10-04 known-limit note is superseded. |
| **E4-E7** | None. Story 4.4's lifecycle test benefits from `owner` on the wire but does not require it. |

No epic becomes obsolete, and no new epic is needed. Epic order is unchanged; E1's new stories precede E2.

### Artifact conflicts

| Artifact | Change |
|---|---|
| PRD `prd.md` FR-3 | "dedicated three-field `Route`" becomes four fields including `owner`. |
| PRD `prd.md` FR-34 | New consequence: the owner is readable through `ListRoutes`. |
| Spine AD-12 | Four fields; "already correct" includes owner equality. `id` and `created_at` stay off the wire. |
| Spine AD-13 | One sentence: the conflict is visible on read. |
| Spine AD-15 | Adds the `tunnel_id` pattern and the `owner` bound. |
| `epics.md` | FR-3 and AD-12 summaries, Story 1.6 AC, Epic 2 dependency notes, Story 2.3 AC, Epic 3 note, Story 3.2 AC, Story 3.4 AC, and the three new stories. |
| UX | None: there is no UX artifact. |
| Secondary | `AGENTS.md` (per story), `Makefile` (`test-db`), `Cargo.toml` (sqlx `migrate` feature for `#[sqlx::test]`), `.sqlx/` (1.9's SELECT changes), `kind/cluster.yaml` comment. No change to the schema, `k8s/`, `compose.yaml` or the Dockerfile. |

### Technical impact

- **Wire:** `Route` gains field 4. That is additive in proto3, and no client outside this repository exists yet (the Go client is Epic 3).
- **Validation:** writes only. Rows already stored keep listing and hydrating (no data migration), and only new creates are checked.
- **New risk created by F-5, handled in 1.8:** with `owner` on the wire and the column unbounded, one row with a multi-megabyte owner would make every `ListRoutes` fail tonic's 4 MiB decode limit, leaving every reconciler with "actual state unknown". 1.8 therefore bounds `owner` at **317 characters** (63 + 1 + 253, the `<namespace>/<name>` maximum Story 1.4 already cites). **This reverses the "no length bound" half of the 2026-10-04 owner decision**; the "no format check" half stands. The column stays `TEXT`.

## 3. Recommended approach

**Direct adjustment (Option 1): viable, selected.** Three Rust stories inside Epic 1, plus documentation edits. Effort: Medium overall (1.8 Low-Medium, 1.9 Low, 1.10 Medium). Risk: Low; every change is additive or narrows input.

- **Rollback (Option 2): not viable.** Nothing shipped is wrong, and the contracts only need extending.
- **MVP review (Option 3): not needed.** No FR is cut, and FR-3 and FR-34 gain one consequence each.

Order: **1.8 → 1.9 → 1.10.** 1.8 fixes the bounds that 1.9's wire field relies on. 1.10 comes last so its harness covers `owner` on the wire as well. Epic 2 starts after 1.10.

## 4. Detailed change proposals

### 4.1 Architecture spine (`architecture/…/ARCHITECTURE-SPINE.md`)

**AD-12: heading and Rule**

OLD:
> ### AD-12 — `ListRoutes` returns a dedicated message, and equality is over two fields
> …
> - **Rule:** `ListRoutes` returns `repeated Route`, a **new** message with exactly three fields: `local_ip`, `tunnel_id`, `remote_endpoint`. … Columns that exist for bookkeeping — `id`, `created_at`, `owner` — are **not** on the wire. "Already correct" is exact string comparison over `tunnel_id` and `remote_endpoint` only.

NEW:
> ### AD-12 — `ListRoutes` returns a dedicated message, and equality is over named fields
> …
> - **Rule:** `ListRoutes` returns `repeated Route`, a **new** message with exactly four fields: `local_ip`, `tunnel_id`, `remote_endpoint`, `owner`. … Columns that exist for bookkeeping — `id`, `created_at` — are **not** on the wire. "Already correct" means `owner` equals the caller's `<namespace>/<name>` **and** `tunnel_id` and `remote_endpoint` equal the spec, by exact string comparison. A row whose values match but whose `owner` differs is a conflict (AD-13), not convergence. *(Amended 2026-10-07, Epic 1 retro F-5: `owner` was off the wire until then.)*

Rationale: the drift hazard AD-12 prevents comes from `id` and `created_at`. On the caller's own rows `owner` always equals the caller, so including it never turns a correct tunnel into drift. It differs only on rows that should be flagged.

**AD-13: Rule, last sentence**

OLD: `A conflicting owner surfaces as a permanent Ready=False reason, not a retry.`
NEW: `A conflicting owner surfaces as a permanent Ready=False reason, not a retry — visible either from Create's FAILED_PRECONDITION or, without any write, from the owner ListRoutes returns (AD-12).`

**AD-15: Rule**

OLD: `… the gateway mirrors the same bounds: local_ip is IPv4, tunnel_id and remote_endpoint are bounded at the column width, remote_endpoint is host:port.`
NEW: `… the gateway mirrors the same bounds: local_ip is IPv4; tunnel_id matches ^[A-Za-z0-9][A-Za-z0-9._-]*$ and is 1 to 255 characters (the column width); remote_endpoint is host:port at most 255 characters. The gateway additionally bounds owner, which no CRD field carries: non-empty, at most 317 characters, no control characters (U+0000–U+001F, U+007F–U+009F).`

### 4.2 PRD (`prds/…/prd.md`)

**FR-3, fourth consequence**

OLD: `The response is a dedicated three-field Route message — local_ip, tunnel_id, remote_endpoint — keyed on local IP, … Bookkeeping columns — id, created_at, owner — are not on the wire, so a whole-message comparison cannot make an already-correct Tunnel look like Drift (AD-12).`
NEW: `The response is a dedicated four-field Route message — local_ip, tunnel_id, remote_endpoint, owner — keyed on local IP, … Bookkeeping columns — id, created_at — are not on the wire, so a comparison cannot make an already-correct Tunnel look like Drift (AD-12). [Amended 2026-10-07: owner added.]`

**FR-34: new consequence, after "The conflict surfaces as a permanent Ready=False reason, not a retry."**

ADD: `- ListRoutes returns each row's owner, so the Operator sees a conflict on read, before any write. [Added 2026-10-07.]`

### 4.3 Epics (`epics.md`)

**Requirements inventory, FR-3 line (`epics.md:34`)**: "a dedicated three-field `Route` message" becomes "a dedicated four-field `Route` message carrying `owner`".

**Additional requirements, AD-12 line (`epics.md:110`)**

OLD: `… a new three-field message keyed local_ip. … Bookkeeping columns — id, created_at, owner — are not on the wire. "Already correct" is exact string comparison over tunnel_id and remote_endpoint only.`
NEW: `… a new four-field message keyed local_ip, carrying owner. … Bookkeeping columns — id, created_at — are not on the wire. "Already correct" is owner equal to the caller's and exact string equality of tunnel_id and remote_endpoint.`

**Story 1.6, AC 1 and 2**

OLD:
> **Then** it returns `repeated Route`, a new message with exactly three fields — `local_ip`, `tunnel_id`, `remote_endpoint` — keyed on `local_ip`,
> …
> **Given** the bookkeeping columns `id`, `created_at` and `owner`, **When** a `Route` is serialized, **Then** none of them appear on the wire …

NEW:
> **Then** it returns `repeated Route`, a new message with exactly three fields — `local_ip`, `tunnel_id`, `remote_endpoint` — keyed on `local_ip` *(Story 1.9 adds `owner` as a fourth field)*,
> …
> **Given** the bookkeeping columns `id` and `created_at`, **When** a `Route` is serialized, **Then** neither appears on the wire … *(`owner` was excluded here as built, and Story 1.9 puts it on the wire, 2026-10-07.)*

Rationale: Story 1.6 is `done`. Annotating its ACs keeps them true to what was built and points forward to 1.9, without rewriting history.

**Epic 2 header and Epic List line (`epics.md:206, 537`)**: "depends on E1 (Story 1.5's validation bounds)" becomes "depends on E1 (the validation bounds of Stories 1.5 and 1.8)". The 2026-10-04 dependency-correction paragraph (`epics.md:194`) gets the same "and 1.8".

**Story 2.3, bounds AC**

OLD:
> **Given** the bounds Story 1.5 recorded for the gateway, **When** the markers are written, **Then** `tunnelID` and `remoteEndpoint` are bounded at the same column widths the gateway enforces, **And** the two validators cannot disagree …

NEW:
> **Given** the bounds Stories 1.5 and 1.8 recorded for the gateway, **When** the markers are written, **Then** `tunnelID` and `remoteEndpoint` are bounded at the same column widths the gateway enforces, **And** `tunnelID` carries the pattern `^[A-Za-z0-9][A-Za-z0-9._-]*$`, so a NUL, a newline or a space is rejected at `kubectl apply`, **And** the two validators cannot disagree …

**Epic 3 decisions, known-limit bullet (`epics.md:698`)**

OLD: `- **Known limit, accepted 2026-10-04:** ListRoutes carries no owner (AD-12). So a second VpnTunnel whose spec exactly matches … Converged means the Gateway holds the declared route, not that this resource owns it.`
NEW: `- **Superseded 2026-10-07 (Epic 1 retro F-5):** the 2026-10-04 known limit — ListRoutes carrying no owner — is removed by Story 1.9. A row that matches the spec but is held by another owner is now a conflict seen on read: Converged means this resource owns the declared route.`

**Story 3.2: replace the already-correct AC and add one**

OLD:
> **Given** a route that matches the spec on `tunnel_id` and `remote_endpoint` by exact string comparison, **When** Reconcile runs, **Then** the fake records zero `Create` and zero `Delete` calls.

NEW:
> **Given** a route whose `owner` is this resource's `<namespace>/<name>` and which matches the spec on `tunnel_id` and `remote_endpoint` by exact string comparison, **When** Reconcile runs, **Then** the fake records zero `Create` and zero `Delete` calls.
>
> **Given** a route for `spec.localIP` whose `owner` is a different resource, whatever its `tunnel_id` and `remote_endpoint`, **When** Reconcile runs, **Then** the fake records zero `Create` and zero `Delete` calls, **And** the pass reports the conflict through Story 3.4's `OwnedByAnother` reason.

**Story 3.4: conflict AC**

OLD: `**Given** a conflict error (FAILED_PRECONDITION), **When** Reconcile handles it, …`
NEW: `**Given** a conflict — a FAILED_PRECONDITION from Create, or a route listed under another owner (Story 3.2) — **When** Reconcile handles it, …` (Then/And unchanged.)

### 4.4 New stories (inserted in `epics.md` after Story 1.7)

#### Story 1.8: Bound what a tunnel identifier and an owner may contain

As the operator's author,
I want the gateway to refuse identifiers that Postgres cannot store or a log cannot show safely,
So that a bad value fails at `kubectl apply` or at validation, never inside the database, and the CRD can mirror one exact rule.

*Realizes AD-15 as amended 2026-10-07. Closes retro findings F-9, F-10, F-11, F-12 and the comment fixes F-2/F-8, and deferrals 1.5 #2 and 1.1 #5.*

**Acceptance Criteria:**

**Given** a create whose `tunnel_id` matches `^[A-Za-z0-9][A-Za-z0-9._-]*$` and is 1 to 255 characters,
**When** it is validated,
**Then** it passes,
**And** any other `tunnel_id` (one containing NUL, CR, LF, a space or a non-ASCII character, or starting with `.`, `_` or `-`) returns `INVALID_ARGUMENT` before any write, with a message that does not echo the value.

**Given** a create or delete whose `owner` contains a control character (U+0000–U+001F, U+007F–U+009F) or is longer than 317 characters,
**When** it is validated,
**Then** it returns `INVALID_ARGUMENT` before any write,
**And** its format is otherwise unchecked (the 2026-10-04 decision stands), and the column stays `TEXT`.

**Given** rows stored before this story whose `tunnel_id` breaks the new pattern,
**When** the gateway hydrates or serves `ListRoutes`,
**Then** they are still loaded and listed, because the bound applies to writes only.

**Given** the five-place rule in `src/services/validation.rs`,
**When** the bounds change,
**Then** the consts, the module doc, the `proto/gateway.proto` comments and the Story 2.3 AC all state the same pattern and lengths, and the `owner` bound is stated in the gateway and the proto only, because no CRD field carries it.

**Given** the two schema copies,
**When** `make test` runs,
**Then** an offline test asserts that `migrations/01_init_routing_table.sql` and `k8s/12-configmap-initdb.yaml` define the same `vpn_routes` columns, and that the `VARCHAR` widths equal the validation consts.

**Given** a create or delete with `local_ip` `"10.0.0.999"`, `"010.0.0.5"` or `" 10.0.0.5"`,
**When** it reaches the handler,
**Then** it returns `INVALID_ARGUMENT`, not `UNAVAILABLE`, and the routing table is unchanged,
**And** a request invalid in every field reports the `local_ip` failure first.

**Given** a store that cannot reach Postgres,
**When** a call fails,
**Then** a test asserts it fails `Unavailable` within 2 seconds, so a dropped acquire timeout fails `make test` instead of only slowing it.

**Given** the retro's comment findings,
**When** this story lands,
**Then** the `kind/cluster.yaml` header names `examples/list_routes.rs`, and `AGENTS.md` says `hydrate()` lives in `src/services/gateway.rs`.

#### Story 1.9: Expose route ownership on `ListRoutes`

As the operator's author,
I want every listed route to carry its owner,
So that a reconciler tells "converged and mine" from "matches, but someone else's" without making a write.

*Realizes FR-3 and FR-34 as amended 2026-10-07, governed by AD-12 and AD-13 as amended. Closes retro finding F-5, and with it the orphan-visibility finding.*

**Acceptance Criteria:**

**Given** `proto/gateway.proto`,
**When** this story lands,
**Then** `Route` gains `string owner = 4`, and `id` and `created_at` stay off the wire,
**And** the proto comment defines "already correct" as `owner` equal to the caller's and `tunnel_id` and `remote_endpoint` equal by exact string, and calls a matching row under another owner a conflict.

**Given** `Store::list_routes`,
**When** it reads `vpn_routes`,
**Then** it selects `owner`, and the regenerated `.sqlx/` lands in the same commit,
**And** `ListRoutes` copies it verbatim. Hydration and the `RoutingTable` are unchanged: the cache does not hold owners.

**Given** the proto's size note,
**When** it is restated,
**Then** it gives the ceiling at maximum widths in bytes: `tunnel_id` 255 ASCII, `remote_endpoint` 255 ASCII, `owner` 317 characters of up to 4 bytes each. That is roughly 2,300 routes under tonic's 4 MiB default.

**Given** `make routes`,
**When** it prints actual state,
**Then** each line shows the owner, escaped like the other fields.

**Given** `make smoke`,
**When** it runs,
**Then** it asserts that `ListRoutes` shows the smoke client's owner on its own row, and still shows that owner after the refused foreign-owner create.

#### Story 1.10: Prove the store's SQL against a real database

As the gateway's maintainer,
I want the owner-guarded upsert, the owner-scoped delete and `ListRoutes` tested automatically against Postgres,
So that a regression in the SQL fails a check instead of waiting for someone to run `make smoke`.

*Closes retro finding F-20: deferrals 1.2 #1-3, 1.3 #8, 1.4 #5, 1.5 #17 and 1.6 #12.*

**Acceptance Criteria:**

**Given** a running Postgres from `make db-up`,
**When** `make test-db` runs,
**Then** `#[sqlx::test]` tests run, each on a fresh database built from `migrations/`,
**And** `make test` stays offline and does not run them.

**Given** the handlers over a real store,
**When** the tests run,
**Then** they prove each of the following:
- a create stores the row and caches the route;
- an owner's second create updates its row and cache;
- a foreign-owner create returns `FAILED_PRECONDITION` and leaves the row, its owner and the cache unchanged;
- a foreign-owner delete returns `existed=false` and keeps the cached route;
- an owner's delete removes the row and clears the cache;
- concurrent creates by two owners for one `local_ip`, over many rounds, always produce exactly one winner;
- `ListRoutes` returns the database's rows, with owners, when the cache disagrees;
- an empty table returns OK and an empty list.

**Given** `AGENTS.md`,
**When** this story lands,
**Then** it lists `make test-db` as a fourth check that needs Postgres, and drops the claim that no unit test reaches a working database.

**Given** `deferred-work.md`,
**When** this story lands,
**Then** the entries it closes are marked resolved.

### 4.5 Sprint status (`implementation-artifacts/sprint-status.yaml`)

Through `bmad-sprint-planning` (or the sprint-status script), never by hand:
- `epic-1: done` becomes `in-progress`
- add `1-8-bound-what-a-tunnel-identifier-and-an-owner-may-contain: backlog`
- add `1-9-expose-route-ownership-on-listroutes: backlog`
- add `1-10-prove-the-store-s-sql-against-a-real-database: backlog`
- `epic-1-retrospective` stays `done`.

## 5. Implementation handoff

**Classification: Moderate.** The backlog changes and planning edits are applied first, then the developer loop runs.

| Step | Who | What |
|---|---|---|
| 1 | Lilian + Claude | Apply §4.1-§4.4 to the spine, PRD and `epics.md`, and §4.5 to sprint status, on a short-lived `docs/` branch (AGENTS.md policy: planning artifacts need no review cycle). |
| 2 | `bmad-build 1.8`, then `1.9`, then `1.10` | One feature branch per story; code commit then `docs:` commit; Lilian rebases on GitHub. |
| 3 | Lilian | Start Epic 2 (Story 2.1, the Go install) after 1.10 is `done`. |

**Success criteria**

- `make check`, `make test`, `make check-sqlx`, `make smoke` and the new `make test-db` all pass.
- `grep -rn sqlx src/ --exclude-dir=store` and `grep -rn println src/` stay empty.
- A NUL or CR/LF `tunnel_id` is refused by validation (no Postgres text in the `Status`). `make routes` shows owners.
- Story 2.3 and Story 3.2/3.4 ACs read as amended, so Epics 2 and 3 are specced against the new contract.
