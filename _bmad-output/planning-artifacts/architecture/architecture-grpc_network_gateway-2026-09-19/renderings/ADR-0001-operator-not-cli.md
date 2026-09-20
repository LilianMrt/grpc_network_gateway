# ADR-0001 — An operator, not a CLI

- **Status:** Accepted · 2026-09-20
- **Governs:** FR-13 to FR-20, FR-32 · epics E2, E3, E4

## Context

The gateway already has a complete imperative API: `CreateVpnTunnel`, `DeleteVpnTunnel`,
`GetGatewayStatus`. Anything a user needs to do, they can already do by calling it. The obvious way
to make that pleasant is a CLI — `gwctl apply tunnels.yaml` — which is a weekend of work and no new
infrastructure.

## Decision

Build a Kubernetes operator instead: a `VpnTunnel` custom resource plus a `controller-runtime`
reconciler that converges the cluster's declared tunnels onto the gateway.

## Rejected alternative — a CLI

A CLI is **edge-triggered**. It acts when invoked, and whatever the user forgets to run simply never
happens. It cannot notice that a route was deleted behind its back, cannot retry a failure that
occurs after it exits, and has nowhere to record that the world does not yet match the intent. Every
one of those gaps is the user's problem to remember.

An operator is **level-triggered**. It re-examines the world on a timer, converges from whatever it
finds rather than from what it expected, and records progress on the resource itself. The difference
is not ergonomics — it is that one of them keeps being true after you stop typing.

## Consequences

- A whole control loop exists to build and test: finalizers, status conditions, requeue behaviour,
  drift correction. That is the cost, and it is most of the project.
- The system gains a property a CLI cannot have: a route deleted out of band comes back
  (UJ-2), with no event to react to.
- `kubectl get vpntunnels` becomes the interface; no bespoke client to distribute or version.
- A CLI remains trivially addable later — it would be a thin client of the same gRPC API — and is
  explicitly not excluded.
