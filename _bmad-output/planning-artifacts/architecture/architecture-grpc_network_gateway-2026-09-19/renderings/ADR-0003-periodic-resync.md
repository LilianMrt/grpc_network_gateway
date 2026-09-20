# ADR-0003 — Periodic resync is not optional

- **Status:** Accepted · 2026-09-20
- **Governs:** FR-17, FR-32 · AD-6, AD-12, AD-14

## Context

`controller-runtime` delivers an event whenever a watched resource changes. It is tempting to treat
those events as the whole input: reconcile when something changed, sleep otherwise. That is what an
event-driven service normally does, and it is cheaper.

## Decision

Every `Reconcile` returns a requeue-after interval — 30 seconds by default, a Helm value — so each
`VpnTunnel` is re-examined on a bounded schedule whether or not anything happened. Each pass reads
actual state fresh from `ListRoutes` and retains nothing about the gateway between passes.

## Rejected alternative — react to events only

The watch only sees **Kubernetes**. It cannot see the gateway. A tunnel deleted directly through the
gateway's own API produces no cluster event, so an event-only loop never learns that the world
stopped matching the spec — and the resource keeps reporting `Ready` while being wrong. The same
applies to any write that failed silently, any state lost with a pod, and any actor other than the
operator touching the external system.

Reacting to events means converging from **what you expected to find**. Resyncing means converging
from **what is actually there**. Only the second is a reconciler; the first is a message handler
with extra steps.

## Consequences

- UJ-2 works: delete a route behind the operator's back and it returns within 30 seconds, with no
  event involved. This is the demo, and it is only possible because of this decision.
- Nothing gateway-derived may be cached between passes (AD-6) — a cache would reintroduce exactly
  the stale expectation this avoids. Connections are exempt: transport is not state.
- A no-op pass must be genuinely free, or a 30-second timer becomes a 30-second write storm. Hence
  AD-12: equality is compared over `tunnel_id` and `remote_endpoint` only, and an already-correct
  tunnel issues zero write calls — asserted by call count, not by outcome.
- 30 seconds is chosen so the demo is watchable unedited. It is far below any load this system
  would notice, and is a chart value rather than a constant.
