# Polyforge Rust SDK documentation

## Shared architecture

- [`adr/SDK-ADR-001-sdk-architecture-and-parity.md`](./adr/SDK-ADR-001-sdk-architecture-and-parity.md) — shared architecture, Core/Cloud target separation, parity, security and conformance rules for all three SDKs.

## ADR ownership

- Shared SDK decisions use `SDK-ADR-*` and must remain logically synchronized across TypeScript, Python and Rust repositories.
- Rust-only implementation decisions use `RS-ADR-*`.
- Core runtime contracts are owned by `F4CTE/PolyForge-core`.
- Cloud/control-plane contracts are owned by `F4CTE/PolyForge`.

## Parity rule

A public feature is complete only when all three SDKs implement the canonical contract or an explicit temporary parity exception exists.
