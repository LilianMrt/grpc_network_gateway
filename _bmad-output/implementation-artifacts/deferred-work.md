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
