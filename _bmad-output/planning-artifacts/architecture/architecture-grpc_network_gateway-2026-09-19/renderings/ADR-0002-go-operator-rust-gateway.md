# ADR-0002 — Go for the operator, Rust for the gateway

- **Status:** Accepted · 2026-09-20
- **Governs:** the repository split · AD-4, AD-5, AD-7

## Context

The gateway is written in Rust and works. The operator does not exist yet. Writing it in Rust with
`kube-rs` would keep one language, one toolchain, one build, and one test harness in the repository.

## Decision

Write the operator in Go with `controller-runtime`, in the same repository under `operator/`, as a
gRPC **client** of the Rust gateway. `proto/gateway.proto` is the only thing they share.

## Rejected alternative — Rust with `kube-rs`

`kube-rs` is a capable library, and a single-language repository is genuinely simpler. Three things
outweigh it:

1. **`controller-runtime` is the thing itself.** It is what the Kubernetes ecosystem is written in,
   and what the reviewers this artifact targets read every day. An operator written in it is legible
   to them at a glance; one written in `kube-rs` invites a conversation about the library instead of
   about the loop.
2. **It deepens the weaker claim.** Rust is already the stronger of the two language claims.
   Reinforcing it buys less than making the Go claim real.
3. **The split is the more honest architecture.** A real cloud control plane separates orchestration
   from the data plane it orchestrates, and they are frequently not the same language. Making the
   operator a module inside the gateway would collapse exactly the boundary the project is meant to
   demonstrate.

## Consequences

- Two toolchains, two test harnesses, two build paths in one Makefile. Real cost, accepted.
- One contract with two consumers, so it needs a rule: `proto/gateway.proto` is the single source,
  Go stubs are committed, and any `.proto` edit regenerates both in the same commit (AD-4).
- The boundary is enforceable rather than aspirational — the operator reaches tunnel state only
  through gRPC and never touches Postgres (AD-7).
- Go must be installed before any of this starts. It currently is not.
