# SDK-ADR-001 — SDK Architecture and Parity

- **Status:** Accepted
- **Date:** 2026-08-04
- **Scope:** `polyforge-sdk-ts`, `polyforge-sdk-python`, `polyforge-sdk-rust`
- **Shared revision:** 3

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
- local/private Prediction Providers and Belief Streams;
- authorized cached commercial Belief Streams;
- strategies and portable `.polyforge` packages;
- backtests, paper trading and live execution;
- runners and deployment targets;
- orders, positions, portfolio and risk;
- whale tracking and copy trading;
- local MCP, webhooks and runtime events;
- installation-local administration.

### Polyforge Cloud

Cloud clients cover operated-service capabilities, including:

- accounts and installation identity;
- marketplace and publisher operations;
- commercial Prediction Provider ingestion and history;
- Belief Stream synchronization, scoring and revocation;
- subscriptions, billing and entitlements;
- Cloud Runners;
- managed backups and restore;
- notifications and monitoring;
- paid remote MCP relay configuration.

Core and Cloud have distinct base URLs, credentials, permissions and availability assumptions. A convenience client may hold both clients but must not hide the target or merge authorization boundaries.

## Canonical contract ownership

- Core runtime contracts are owned by `F4CTE/PolyForge-core` ADRs, schemas and APIs.
- Cloud commercial/control-plane contracts are owned by `F4CTE/PolyForge` ADRs, schemas and APIs.
- `PREDICTION_PROVIDERS.md` is the normative shared Prediction Provider and Belief Stream wire specification.
- The SDKs consume generated or validated forms of canonical contracts.
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
5. streaming, pagination, cancellation, idempotency, sequence and freshness semantics are represented consistently.

## Venue Provider conformance

The SDKs consume `CORE-ADR-003 — Venue Provider Architecture` and expose equivalent representations for:

- `VenueId`, `VenueMarketRef` and canonical mappings;
- `VenueAccountRef`, including wallet, account and subaccount identity;
- typed manifests and capabilities;
- cursor-based pages with authoritative/completeness metadata;
- ordered market-data snapshots and deltas;
- freshness and sequence-gap metadata;
- `VenueOrderIntent` and decimal-string financial values;
- accepted, rejected and unknown placement outcomes;
- single and scoped paginated cancellation results;
- fills, fees, positions and reconciliation results;
- stable venue errors and readiness states.

The SDKs preserve these guarantees:

1. Unsupported order types are never silently converted.
2. Account/subaccount identity survives placement, cancellation, retrieval and reconciliation.
3. Incomplete or non-authoritative pages are not exposed as authoritative empty results.
4. Ambiguous placement remains `UNKNOWN` until reconciled.
5. Cancel-all exposes unresolved orders and completion state.
6. Canonical financial values remain decimal strings.
7. Streaming exposes gap, reconnect and freshness information when available.

## Belief Stream conformance

The SDKs consume `CORE-ADR-004`, `CLOUD-ADR-003` and the normative `PREDICTION_PROVIDERS.md` specification.

At minimum, parity covers:

- `BeliefStreamRef`;
- `PredictionRevision`;
- active, abstained, withdrawn and invalidated states;
- canonical decimal-string probability and confidence;
- authoritative `sequence`, `receivedAt`, `contentHash` and `sourcePlane`;
- Provider `generatedAt`, `validUntil` and model version;
- corrections through `supersedesPredictionId`;
- structured drivers, risks, sources and bounded extensions;
- `BeliefStreamState`, freshness and derived metrics;
- synchronization cursors, completeness and sequence gaps;
- immutable revision history;
- private local publication and commercial Cloud synchronization;
- strategy-facing freshness and failure policies;
- entitlement, revocation and offline-grace metadata where applicable;
- versioned Prediction and Belief Stream events.

The SDKs preserve these behavioral guarantees:

1. Accepted revisions are immutable; helpers never update or delete history in place.
2. Corrections, abstentions, withdrawals and invalidations are explicit new records or ledger-linked events.
3. A later revision with an unresolved sequence gap is not represented as complete authoritative current state.
4. Duplicate IDs with conflicting hashes are surfaced as integrity errors.
5. Missing, stale, expired, withdrawn or gap-affected predictions are never converted to zero probability or confidence.
6. Canonical probability, confidence, impact and score values remain decimal strings; convenience conversions are explicitly non-authoritative.
7. Core and Cloud receipt/sequence authority remains visible through `sourcePlane` and target-specific clients.
8. Commercial synchronization keeps entitlement scope, cursor, completeness and revocation explicit.
9. Text summaries are exposed for humans but SDK helpers do not parse prose into executable values.
10. Event streams are idempotent and expose schema version, stream identity, prediction ID and sequence.

Core client operations include listing streams, retrieving current state and history, subscribing to events, inspecting gaps/freshness, and administering private Providers.

Cloud client operations include publisher ingestion status, commercial history, entitlements, synchronization cursors, scoring, moderation and revocation.

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

All three represent the same deadlines, cursor state, completeness, ambiguity, sequence gaps, freshness and reconciliation requirements.

## Security rules

- Raw credentials are never serialized into strategies, Provider manifests or Runner packages.
- Core and Cloud tokens are not interchangeable.
- Convenience APIs cannot silently forward local data to Cloud.
- Remote MCP relay operations remain explicit paid Cloud operations.
- Sensitive values must be redacted from errors and logs consistently across SDKs.
- Venue connection references may be transmitted, but resolved secrets and signed private payloads are not returned through ordinary SDK models.
- Publisher ingestion credentials are exposed only by Cloud administration operations and are never mixed with subscriber or Core credentials.
- SDK telemetry must not include prediction payloads, strategies, wallets or positions by default.

## Versioning

SDK package versions may advance independently, but each release declares:

- supported Core API/protocol versions;
- supported Cloud API/protocol versions;
- Provider protocol versions;
- Venue Provider contract versions;
- Prediction Provider/Belief Stream specification versions;
- event schema versions;
- known temporary parity gaps.

Breaking canonical contract changes require a compatible SDK major-version strategy or an explicit negotiated protocol transition.

## Conformance

The repositories should share:

- request/response and error fixtures;
- pagination, cursor and streaming cases;
- Provider and Venue Provider manifests;
- order-type, account isolation, ambiguous placement and reconciliation cases;
- active, abstained, withdrawn and invalidated prediction revisions;
- decimal boundary and precision cases;
- correction and supersession cases;
- duplicate identity and conflicting hash cases;
- sequence-gap and resynchronization cases;
- stale and expired state cases;
- structured driver, risk, source and extension limits;
- commercial entitlement, cursor and revocation cases;
- portable strategy examples;
- compatibility matrices.

CI should detect drift between published schemas and language implementations.

## Related decisions

- `CORE-ADR-001` and `CLOUD-ADR-001` define product boundaries.
- `CORE-ADR-002` defines Provider runtime contracts.
- `CORE-ADR-003` defines Venue Provider semantics.
- `CLOUD-ADR-002` defines commercial Provider services.
- `CORE-ADR-004` defines Belief Stream runtime semantics.
- `CLOUD-ADR-003` defines commercial Belief Stream ingestion, scoring and distribution.
