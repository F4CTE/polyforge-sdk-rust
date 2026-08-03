# Polyforge Rust SDK documentation

## Shared architecture

- [`adr/SDK-ADR-001-sdk-architecture-and-parity.md`](./adr/SDK-ADR-001-sdk-architecture-and-parity.md) — shared architecture, Core/Cloud target separation, parity, security and conformance rules for all three SDKs. Current shared revision: **2**.

The shared ADR consumes:

- `CORE-ADR-002` for Provider runtime contracts;
- `CORE-ADR-003` for Venue Provider order, account, cancellation, market-data and reconciliation semantics;
- `CLOUD-ADR-001` and `CLOUD-ADR-002` for control-plane and commercial-service behavior.

## ADR ownership

- Shared SDK decisions use `SDK-ADR-*` and must remain logically synchronized across TypeScript, Python and Rust repositories.
- Rust-only implementation decisions use `RS-ADR-*`.
- Core runtime contracts are owned by `F4CTE/PolyForge-core`.
- Cloud/control-plane contracts are owned by `F4CTE/PolyForge`.

## Parity rule

A public feature is complete only when all three SDKs implement the canonical contract or an explicit temporary parity exception exists.
