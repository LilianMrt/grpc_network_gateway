# PRDs — `grpc_network_gateway`

Index of product requirement documents. Downstream workflows discover the PRD through this file.

## Current

- **[grpc_network_gateway — declarative control plane](prd-grpc_network_gateway-2026-09-19/prd.md)** — status `final`, updated 2026-09-19.
  The Rust gRPC gateway plus the Go Kubernetes operator that reconciles `VpnTunnel` custom resources against it. 32 FRs; 22 to build, sequenced as 6 epics. **§9 Build Order and Phasing is the authority on epic sequence, and every FR carries a binding `Status:` line — `Delivered — no story` FRs must not produce stories.**
  - Companion: [addendum.md](prd-grpc_network_gateway-2026-09-19/addendum.md) — rejected alternatives, verification evidence, host constraints, README/ADR framing, milestone mapping.
  - Reviews: `review-rubric.md`, `review-technical-accuracy.md`, `review-downstream.md`, `reconcile-build-plan.md` in the same folder.
