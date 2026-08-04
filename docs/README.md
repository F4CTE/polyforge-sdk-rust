# Polyforge Rust SDK documentation

## Shared architecture

- [`adr/SDK-ADR-001-sdk-architecture-and-parity.md`](./adr/SDK-ADR-001-sdk-architecture-and-parity.md) — shared Core/Cloud target separation, parity, security and conformance rules. Current shared revision: **9**.

The shared ADR consumes:

- `CORE-ADR-002` for Provider runtime contracts;
- `CORE-ADR-003` for Venue Provider semantics;
- `CORE-ADR-004` and `CLOUD-ADR-003` for Prediction Providers and Belief Streams;
- `CORE-ADR-005` and `CLOUD-ADR-004` for Runner runtime and Cloud Runner behavior;
- `CORE-ADR-006` and `CLOUD-ADR-005` for Core/Cloud MCP and remote relay;
- `CORE-ADR-007` and `CLOUD-ADR-006` for Marketplace installation, commerce, entitlements and settlement;
- `CORE-ADR-008` and `CLOUD-ADR-007` for connections, secret lifecycle, signer isolation and managed custody;
- `CORE-ADR-009` and `CLOUD-ADR-008` for backup, restore and disaster recovery;
- `CORE-ADR-010` and `CLOUD-ADR-009` for releases, upgrades, migrations and managed updates;
- all seven normative shared specifications from `PREDICTION_PROVIDERS.md` through `UPDATE_AND_MIGRATION_PROTOCOL.md`.

## ADR ownership

- Shared SDK decisions use `SDK-ADR-*` and remain logically synchronized across TypeScript, Python and Rust repositories.
- Rust-only implementation decisions use `RS-ADR-*`.
- Core runtime contracts are owned by `F4CTE/PolyForge-core`.
- Cloud/control-plane contracts are owned by `F4CTE/PolyForge`.

## Parity rule

A public feature is complete only when all three SDKs implement the canonical contract or an explicit temporary parity exception exists.
