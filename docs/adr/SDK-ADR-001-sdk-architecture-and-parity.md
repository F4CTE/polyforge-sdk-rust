# SDK-ADR-001 — SDK Architecture and Parity

- **Status:** Accepted
- **Date:** 2026-08-04
- **Scope:** `polyforge-sdk-ts`, `polyforge-sdk-python`, `polyforge-sdk-rust`
- **Shared revision:** 5

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
- execution-package compilation and inspection;
- local and user-operated remote Runners;
- Runner deployments, commands, health, events, checkpoints and migration;
- local MCP catalog, credentials, tool policy, approvals and operations;
- Core relay grants, tunnel status and local relay activity;
- orders, positions, portfolio and risk;
- whale tracking and copy trading;
- local webhooks and runtime events;
- installation-local administration.

### Polyforge Cloud

Cloud clients cover operated-service capabilities, including:

- accounts and installation identity;
- marketplace and publisher operations;
- commercial Prediction Provider ingestion and history;
- Belief Stream synchronization, scoring and revocation;
- subscriptions, billing and entitlements;
- Cloud Runner regions, classes, admission, deployment and usage;
- Cloud Runner connection provisioning, commands, checkpoints and migration;
- Cloud MCP catalog, OAuth metadata and Cloud-owned operations;
- remote MCP relay subscriptions, grants, tunnel status, invocation metadata and revocation;
- managed backups and restore;
- notifications and monitoring.

Public SDK clients do not expose private admin MCP tools by default.

Core and Cloud have distinct base URLs, credentials, permissions and availability assumptions. A convenience client may hold both clients but must not hide the target or merge authorization boundaries.

## Canonical contract ownership

- Core runtime contracts are owned by `F4CTE/PolyForge-core` ADRs, schemas and APIs.
- Cloud commercial/control-plane contracts are owned by `F4CTE/PolyForge` ADRs, schemas and APIs.
- `PREDICTION_PROVIDERS.md` is the normative shared Prediction Provider and Belief Stream wire specification.
- `RUNNER_PROTOCOL.md` is the normative shared Runner manifest, package, deployment, lease, command, event and checkpoint specification.
- `MCP_PROTOCOL.md` is the normative shared MCP descriptor, catalog, call, result, approval, operation, error and relay specification.
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

An optional MCP transport client may be provided, but it must preserve the same Core/Cloud target separation and cannot treat Cloud credentials as Core credentials.

## General parity requirements

A public capability is complete only when:

1. its canonical contract is versioned;
2. all three SDKs implement it, or a temporary parity exception is recorded;
3. all SDKs pass the same conformance examples and fixtures;
4. Core/Cloud compatibility requirements are documented;
5. streaming, pagination, cancellation, idempotency, approval, scope, sequence, freshness, generation and fencing semantics are represented consistently.

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

## Runner conformance

The SDKs consume `CORE-ADR-005`, `CLOUD-ADR-004` and the normative `RUNNER_PROTOCOL.md` specification.

At minimum, parity covers:

- `RunnerManifest` and typed Runner capabilities;
- Runner resource limits, platform, runtime and Provider compatibility;
- `StrategyExecutionPackage` and content hashes;
- `RunnerDeployment`, desired state and monotonic generation;
- target-specific logical connection bindings;
- `RunnerLease`, expiry, renewal deadline and permitted safety actions;
- canonical deployment states, including `ORPHANED`;
- idempotent `RunnerCommand` operations and command status;
- ordered, versioned `RunnerEvent` streams;
- event sequence gaps, replay cursors and deduplication identity;
- `RunnerCheckpoint`, integrity, encryption and compatibility metadata;
- Runner and deployment health, heartbeat and readiness;
- migration plans, source fencing and target restore;
- Cloud Runner regions, classes, admission, quotas, metering and maintenance metadata.

The SDKs preserve these behavioral guarantees:

1. Execution packages never contain resolved secrets or unrestricted credentials.
2. Package, strategy revision, runtime version and content hash remain explicit and immutable for one deployment.
3. A stale generation or command is represented as fenced/rejected and cannot be hidden by automatic retry.
4. Lease loss remains visible; SDK helpers must not continue or restart risk-increasing execution automatically.
5. Risk-reducing or cancellation permissions after lease expiry are explicit, never assumed.
6. Commands are idempotent by command ID. A timeout or missing acknowledgement is not converted to success.
7. Runner events remain at-least-once, ordered per generation and deduplicable by event ID.
8. Sequence gaps are surfaced and require replay or reconciliation before state is called authoritative.
9. Checkpoints expose package hash, runtime, generation, state hash and event sequence. Incompatible restore is rejected.
10. Live restore and migration require external Provider reconciliation before increasing risk.
11. Migration allocates a newer generation and exposes source-fencing status; helpers never start both generations optimistically.
12. Runner process health, deployment liveness, lease ownership and private Provider readiness remain separate fields.
13. Core and Cloud Runner operations remain on their explicit target clients with distinct credentials and authorization.
14. Cloud usage and billing models do not expose strategy logic, wallets, positions or secret values.

Core client operations include:

- list/register/inspect local and remote Runners;
- validate compatibility;
- compile and inspect execution packages;
- create and control deployments;
- query commands, health, events and sequence gaps;
- create/list/restore checkpoints;
- bind connections and migrate deployments.

Cloud client operations include:

- list Cloud Runner regions, classes and compatibility;
- submit packages for admission;
- provision target-specific connections;
- create/control/migrate Cloud deployments;
- inspect operational health and synchronization status;
- manage checkpoint retention;
- inspect quotas, usage, billing and maintenance windows.

## MCP conformance

The SDKs consume `CORE-ADR-006`, `CLOUD-ADR-005` and the normative `MCP_PROTOCOL.md` specification.

At minimum, parity covers:

- `McpToolDescriptor` and independent Core, Cloud and Admin owner planes;
- catalog and individual tool versions;
- input and output JSON Schemas;
- required scopes and capability requirements;
- effect classes: read, write, financial, destructive and admin;
- approval, idempotency, relay and data-classification policies;
- `McpToolCall` and structured `McpToolResult`;
- stable `McpError`, warnings and correlation IDs;
- `McpApprovalChallenge` and exact payload binding;
- `McpOperationRef` for long-running work;
- Core local credentials and launch profiles;
- Cloud OAuth protected-resource metadata and scope information;
- `McpRelayGrant`, request/response envelopes and tunnel status;
- grant expiry, revocation, offline and replay state;
- redacted MCP audit metadata;
- explicit exclusion of public Admin MCP tools.

The SDKs preserve these behavioral guarantees:

1. `tools/list` filtering is never represented as proof of authorization; invocation may still fail.
2. Core and Cloud catalogs, base URLs, tokens and scopes remain distinct.
3. Cloud OAuth access tokens are never reused as Core credentials.
4. SDKs do not offer a generic arbitrary Core API relay helper.
5. Approval challenges remain bound to exact tool version, payload hash, actor, target, expiry and idempotency identity.
6. A changed semantic payload requires a new approval challenge.
7. Required-idempotency tools reject missing keys; conflicting payloads for one key remain conflicts.
8. A timeout, disconnect or ambiguous result is not converted into a fresh mutation.
9. Long-running tools return an explicit accepted operation and status reference.
10. Structured result fields are authoritative; SDK helpers do not parse human text to infer status or data.
11. Relayed Core calls preserve local Core denial, approval requirement, ambiguity and failure without weakening them.
12. Mutating, financial and destructive relay calls are not queued while Core is offline.
13. Financial relay remains disabled by default and its enabled state is explicit.
14. Admin MCP remains private, separately authenticated and non-relayable.
15. Sensitive arguments, results, secrets and signed payloads are redacted from ordinary SDK logs and telemetry.
16. Correctness does not depend on one MCP transport session or process remaining alive.

Core client operations include:

- list and inspect the local Core MCP catalog;
- create, list and revoke scoped Core MCP credentials or launch profiles where supported;
- inspect tool availability, scopes, effect and approval policy;
- inspect and resolve local approval challenges;
- query asynchronous Core MCP operations;
- configure local relay permissions and inspect tunnel/activity state;
- invoke Core MCP through an optional transport client without changing Core authorization semantics.

Cloud client operations include:

- discover Cloud MCP OAuth protected-resource metadata and catalog versions;
- inspect Cloud tool scopes, entitlements and availability;
- inspect and resolve Cloud-owned approval challenges;
- query asynchronous Cloud MCP operations;
- manage remote-relay subscription state, grants, expiry and revocation;
- inspect relayable Core descriptors, installation tunnel state and redacted activity;
- invoke explicit relayed Core tools through supported transport helpers while preserving relay and local Core errors.

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

All three represent the same deadlines, cursor state, completeness, ambiguity, sequence gaps, freshness, reconciliation, command status, generation, fencing, scope, approval, idempotency, operation and relay requirements.

## Security rules

- Raw credentials are never serialized into strategies, Provider manifests, execution packages or checkpoints.
- Core and Cloud tokens are not interchangeable.
- Convenience APIs cannot silently forward local data to Cloud.
- Remote MCP relay operations remain explicit paid Cloud operations.
- Sensitive values must be redacted from errors and logs consistently across SDKs.
- Venue connection references may be transmitted, but resolved secrets and signed private payloads are not returned through ordinary SDK models.
- Publisher ingestion credentials are exposed only by Cloud administration operations and are never mixed with subscriber or Core credentials.
- Runner connection bindings are opaque references; secret material is never returned by ordinary deployment APIs.
- MCP transport clients must not log bearer credentials, approval secrets, raw signed relay envelopes or unrestricted tool payloads.
- Relay grants and subscriptions never imply local Core authorization.
- SDK telemetry must not include prediction payloads, strategies, wallets, positions, packages, checkpoints, MCP arguments/results or secrets by default.

## Versioning

SDK package versions may advance independently, but each release declares:

- supported Core API/protocol versions;
- supported Cloud API/protocol versions;
- Provider protocol versions;
- Venue Provider contract versions;
- Prediction Provider/Belief Stream specification versions;
- Runner protocol and Core runtime versions;
- MCP protocol, catalog, tool and relay versions;
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
- Belief Stream sequence-gap and resynchronization cases;
- stale and expired state cases;
- structured driver, risk, source and extension limits;
- commercial entitlement, cursor and revocation cases;
- Runner manifest and compatibility cases;
- execution package hash and secret-exclusion cases;
- state transition and idempotent command cases;
- lease expiry, stale generation and fencing cases;
- Runner event replay, deduplication and gap cases;
- checkpoint integrity and incompatible restore cases;
- pause, drain, stop and emergency safety cases;
- source-fenced migration and target-restore cases;
- Cloud admission, quota, usage and maintenance cases;
- MCP descriptor, catalog and scope cases;
- tool invocation authorization independent from catalog filtering;
- approval payload binding and expiry cases;
- idempotency conflict and ambiguous timeout cases;
- asynchronous MCP operation cases;
- OAuth protected-resource metadata and audience cases;
- Core credential and Cloud token separation cases;
- relay grant, expiry, revocation and tool-version cases;
- signed relay request/response and replay rejection cases;
- offline Core and non-queued mutation cases;
- local Core rejection after Cloud relay authorization;
- audit redaction and admin MCP isolation cases;
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
- `CORE-ADR-005` defines Runner runtime, generations, fencing, events, checkpoints and migration.
- `CLOUD-ADR-004` defines the Cloud Runner fleet and control plane.
- `CORE-ADR-006` defines local Core MCP tools, authorization, approval and relay reception.
- `CLOUD-ADR-005` defines Cloud MCP OAuth, remote relay and private admin MCP.
