# SDK-ADR-001 — SDK Architecture and Parity

- **Status:** Accepted
- **Date:** 2026-08-04
- **Scope:** `polyforge-sdk-ts`, `polyforge-sdk-python`, `polyforge-sdk-rust`
- **Shared revision:** 2

## Shared-decision rule

This ADR is shared by the three SDK repositories and must remain logically identical in each implementation repository.

A change is accepted only when:

1. the shared revision is incremented;
2. equivalent ADR changes are prepared for all three repositories;
3. implementation and conformance impact is recorded;
4. temporary parity exceptions are explicitly tracked.

Language-specific architecture decisions use separate prefixes:

- `TS-ADR-*`
- `PY-ADR-*`
- `RS-ADR-*`

## Principle

The TypeScript, Python and Rust Polyforge SDKs expose the same public capabilities and contracts.

Language-native ergonomics may differ, but resource coverage, validation, events, error meaning and authorization boundaries remain equivalent.

## Explicit API targets

Every SDK supports two distinct targets.

### Polyforge Core

Core clients cover runtime and self-hosted capabilities, including:

- markets and Venue Provider capabilities;
- strategies and portable `.polyforge` packages;
- backtests, paper trading and live execution;
- runners and deployment targets;
- orders, positions, portfolio and risk;
- whale tracking and copy trading;
- local/private Providers and cached commercial outputs;
- local MCP, webhooks and runtime events;
- installation-local administration.

### Polyforge Cloud

Cloud clients cover operated-service capabilities, including:

- accounts and installation identity;
- marketplace and publisher operations;
- subscriptions, billing and entitlements;
- Cloud Runners;
- managed backups and restore;
- commercial Provider history and scoring;
- notifications and monitoring;
- paid remote MCP relay configuration.

Core and Cloud have distinct base URLs, credentials, permissions and availability assumptions. A convenience client may hold both clients but must not hide the target or merge authorization boundaries.

## Canonical contract ownership

- Core runtime contracts are owned by `F4CTE/PolyForge-core` ADRs, schemas and APIs.
- Cloud commercial/control-plane contracts are owned by `F4CTE/PolyForge` ADRs, schemas and APIs.
- The SDKs consume generated or validated forms of those contracts.
- SDKs must not invent language-only public domain models that conflict with canonical contracts.

Public types should be generated from or validated against versioned OpenAPI, JSON Schema and event-schema fixtures where practical.

## Client shape

Each SDK provides explicit equivalents of:

```text
PolyforgeCoreClient
PolyforgeCloudClient
```

An optional high-level `PolyforgeClient` may coordinate both but preserves:

- target selection;
- separate authentication;
- explicit data movement;
- independent retries and failure handling;
- distinct permission checks.

## General parity requirements

A public capability is complete only when:

1. its canonical contract is versioned;
2. all three SDKs implement it, or a temporary parity exception is recorded;
3. all SDKs pass the same conformance examples and fixtures;
4. Core/Cloud compatibility requirements are documented;
5. streaming, pagination, cancellation and idempotency semantics are represented consistently.

## Venue Provider conformance

The SDKs consume `CORE-ADR-003 — Venue Provider Architecture` and must expose equivalent representations for the canonical Venue Provider contract.

At minimum, parity covers:

- `VenueId` and extensible future venue identifiers;
- `VenueMarketRef` and canonical-market mappings;
- `VenueAccountRef`, including wallet, account and subaccount identity;
- typed Venue Provider manifests and capabilities;
- cursor-based market, order and position pages;
- authoritative and completeness metadata;
- ordered market-data snapshots and deltas;
- sequence, freshness and stale-data metadata;
- `VenueOrderIntent` and decimal-string financial values;
- accepted, rejected and unknown placement outcomes;
- single and scoped paginated cancellation results;
- fills, fees, positions and reconciliation results;
- stable venue error categories and retry/reconciliation metadata;
- separate public-data and private-execution readiness.

The SDKs must preserve these behavioral guarantees:

1. Unsupported order types are rejected and never silently converted to another type.
2. Account and subaccount identity is preserved across placement, cancellation, retrieval and reconciliation.
3. A page with `complete: false` or `authoritative: false` is not exposed as an authoritative empty result.
4. Ambiguous placement remains `UNKNOWN` until reconciled; SDK helpers cannot silently submit a fresh order.
5. Cancel-all exposes unresolved orders and completion status rather than returning a false success.
6. Financial quantities and prices remain decimal strings unless the caller explicitly opts into a non-authoritative convenience conversion.
7. Market-data streaming exposes gap, reconnect and freshness information when present.
8. Cancellation, timeout and abort behavior maps cleanly to language-native primitives without changing the contract meaning.

Venue-specific extension payloads remain accessible in typed or validated extension containers. SDKs must not discard unknown extension fields required to preserve venue semantics.

## Language-specific freedom

Implementations may differ in:

- async primitives;
- iterator and streaming APIs;
- error wrapper types;
- package/module layout;
- naming adaptations required by language conventions;
- transport implementation details.

These differences must not alter observable contract behavior.

Examples:

- TypeScript may use `Promise`, `AsyncIterable` and `AbortSignal`;
- Python may use async iterators and cancellation through its selected async runtime;
- Rust may use `Future`, `Stream`, typed errors and explicit cancellation tokens.

All three must represent the same deadlines, pagination state, completeness, ambiguity and reconciliation requirements.

## Security rules

- Raw credentials are never serialized into strategies, Provider manifests or Runner packages.
- Core and Cloud tokens are not interchangeable.
- Convenience APIs cannot silently forward local data to Cloud.
- Remote MCP relay operations remain explicit paid Cloud operations.
- Sensitive values must be redacted from errors and logs consistently across SDKs.
- Venue connection references may be transmitted, but resolved secrets and signed private payloads are not returned through ordinary SDK models.

## Versioning

SDK package versions may advance independently, but each release declares:

- supported Core API/protocol versions;
- supported Cloud API/protocol versions;
- Provider protocol versions;
- Venue Provider contract versions;
- event schema versions;
- known temporary parity gaps.

Breaking canonical contract changes require a compatible SDK major-version strategy or an explicit negotiated protocol transition.

## Conformance

The repositories should share:

- request/response fixtures;
- error fixtures;
- pagination cases;
- streaming/event cases;
- Provider manifest examples;
- Venue Provider capability fixtures;
- order-type preservation cases;
- account/subaccount isolation cases;
- ambiguous placement and idempotent retry cases;
- paginated cancel-all cases;
- authoritative reconciliation cases;
- stale-data and sequence-gap cases;
- portable strategy examples;
- compatibility matrices.

CI should detect drift between published schemas and language implementations.

## Related decisions

- `CORE-ADR-001` and `CLOUD-ADR-001` define product boundaries.
- `CORE-ADR-002` defines Provider runtime contracts.
- `CORE-ADR-003` defines Venue Provider execution, market-data and reconciliation semantics.
- `CLOUD-ADR-002` defines commercial Provider services.
