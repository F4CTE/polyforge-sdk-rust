# Polyforge Rust SDK documentation

## Shared architecture

- [`adr/SDK-ADR-001-sdk-architecture-and-parity.md`](./adr/SDK-ADR-001-sdk-architecture-and-parity.md) — shared Core/Cloud target separation, parity, security and conformance rules. Current shared revision: **4**.

The shared ADR consumes:

- `CORE-ADR-002` for Provider runtime contracts;
- `CORE-ADR-003` for Venue Provider semantics;
- `CORE-ADR-004` and `CLOUD-ADR-003` for Prediction Providers and Belief Streams;
- `CORE-ADR-005` and `CLOUD-ADR-004` for Runner runtime and Cloud Runner control-plane behavior;
- `PREDICTION_PROVIDERS.md` and `RUNNER_PROTOCOL.md` as normative shared specifications.

## ADR ownership

- Shared SDK decisions use `SDK-ADR-*` and remain logically synchronized across TypeScript, Python and Rust repositories.
- Rust-only implementation decisions use `RS-ADR-*`.
- Core runtime contracts are owned by `F4CTE/PolyForge-core`.
- Cloud/control-plane contracts are owned by `F4CTE/PolyForge`.

## Parity rule

A public feature is complete only when all three SDKs implement the canonical contract or an explicit temporary parity exception exists.
