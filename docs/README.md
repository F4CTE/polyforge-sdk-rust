# Polyforge Rust SDK documentation

## Shared architecture

- [`SDK-ADR-001`](./adr/SDK-ADR-001-sdk-architecture-and-parity.md) — shared Core/Cloud client architecture, functional parity, Venue Provider conformance and Belief Stream conformance for all three SDKs.

Current shared revision: **3**.

## Canonical dependencies

- `CORE-ADR-002` — Provider runtime contracts.
- `CORE-ADR-003` — Venue Provider behavior.
- `CORE-ADR-004` — Belief Stream runtime behavior.
- `CLOUD-ADR-001` through `CLOUD-ADR-003` — control-plane and commercial-service behavior.
- `F4CTE/PolyForge-core/docs/specs/PREDICTION_PROVIDERS.md` — normative Prediction Provider and Belief Stream wire specification.

## ADR ownership

- Shared SDK decisions use `SDK-ADR-*` and remain logically synchronized across TypeScript, Python and Rust repositories.
- Rust-only implementation decisions use `RS-ADR-*`.
- Core runtime contracts are owned by `F4CTE/PolyForge-core`.
- Cloud/control-plane contracts are owned by `F4CTE/PolyForge`.

## Parity rule

A public feature is complete only when all three SDKs implement the canonical contract or an explicit temporary parity exception exists.
