# SDK-ADR-001 — SDK Architecture and Parity

- **Status:** Accepted
- **Date:** 2026-08-04
- **Scope:** `polyforge-sdk-ts`, `polyforge-sdk-python`, `polyforge-sdk-rust`
- **Shared revision:** 6

## Shared-decision rule

This ADR is shared by the three SDK repositories and must remain logically identical in each implementation repository.

A change is accepted only when:

1. the shared revision is incremented;
2. equivalent ADR changes are prepared for all three repositories;
3. implementation and conformance impact is recorded;
4. temporary parity exceptions are explicitly tracked.

Language-specific architecture decisions use separate prefixes:

- `TS-ADR-*`;
- `PY-ADR-*`;
- `RS-ADR-*`.

## Principle

The TypeScript, Python and Rust Polyforge SDKs expose the same public capabilities and contracts.

Language-native ergonomics may differ, but resource coverage, validation, event meaning, error meaning, authorization boundaries and safety guarantees remain equivalent.

## Explicit API targets

Every SDK supports two distinct targets.

### Polyforge Core

Core clients cover self-hosted and runtime capabilities, including:

- markets and Venue Provider capabilities;
- local/private Providers and cached commercial outputs;
- Prediction Providers and Belief Streams;
- strategies, portable `.polyforge` packages and backtests;
- paper and live execution;
- execution-package compilation and inspection;
- local and user-operated remote Runners;
- Runner deployments, commands, health, events, checkpoints and migration;
- local MCP catalog, credentials, tool policy, approvals and operations;
- Core relay grants, tunnel status and local relay activity;
- Marketplace release inspection, validation and local installation;
- installation receipts, update checks, rollback, quarantine and uninstall;
- orders, positions, portfolio, risk, whale tracking and copy trading;
- local webhooks, runtime events and installation administration.

### Polyforge Cloud

Cloud clients cover commercial and operated-service capabilities, including:

- accounts, workspaces and installation identity;
- Marketplace assets, listings, releases, publishers and reviews;
- offers, acquisition, payment status, entitlements and distribution grants;
- subscription state, refunds and entitlement revocation;
- publisher balances, settlement references and payout status;
- commercial Provider and Belief Stream ingestion, scoring and distribution;
- Cloud Runner regions, classes, admission, deployment and usage;
- Cloud MCP catalog, OAuth metadata and Cloud-owned operations;
- remote MCP relay subscriptions, grants, tunnel status and invocation metadata;
- managed backups, notifications and monitoring.

Public SDK clients do not expose private admin MCP tools or internal financial-ledger mutation primitives by default.

Core and Cloud have distinct base URLs, credentials, permissions and availability assumptions. A convenience `PolyforgeClient` may coordinate both but must not hide the target, merge authorization boundaries or silently move local data to Cloud.

## Canonical contract ownership

- Core runtime contracts are owned by `F4CTE/PolyForge-core` ADRs, schemas and APIs.
- Cloud commercial/control-plane contracts are owned by `F4CTE/PolyForge` ADRs, schemas and APIs.
- `PREDICTION_PROVIDERS.md` is the normative Prediction Provider and Belief Stream contract.
- `RUNNER_PROTOCOL.md` is the normative Runner contract.
- `MCP_PROTOCOL.md` is the normative MCP and remote-relay contract.
- `MARKETPLACE_PROTOCOL.md` is the normative Marketplace asset, release, offer, entitlement, distribution and installation contract.
- SDKs consume generated or validated forms of canonical contracts and must not invent conflicting language-only public domain models.

Public types should be generated from or validated against versioned OpenAPI, JSON Schema and event-schema fixtures where practical.

## Client shape

Each SDK provides explicit equivalents of:

```text
PolyforgeCoreClient
PolyforgeCloudClient
```

An optional MCP transport client may be provided, but it preserves Core/Cloud target separation and cannot treat a Cloud credential as a Core credential.

Language-specific clients may use builders, async iterators, streams or typed option objects as long as contract behavior is unchanged.

## General parity requirements

A public capability is complete only when:

1. its canonical contract is versioned;
2. all three SDKs implement it, or a temporary parity exception is recorded;
3. all SDKs pass shared conformance examples and fixtures;
4. Core/Cloud compatibility requirements are documented;
5. pagination, streaming, cancellation, idempotency, approval, scopes, sequence, freshness, generation, fencing, entitlement and integrity semantics are represented consistently.

## Venue Provider conformance

The SDKs consume `CORE-ADR-003 — Venue Provider Architecture`.

Parity covers:

- `VenueId`, `VenueMarketRef` and canonical mappings;
- `VenueAccountRef`, including wallet, account and subaccount identity;
- typed manifests and capabilities;
- cursor pages with authoritative/completeness metadata;
- ordered market-data snapshots and deltas;
- freshness and sequence gaps;
- `VenueOrderIntent` and decimal-string values;
- accepted, rejected and unknown placement outcomes;
- scoped paginated cancellation;
- fills, fees, positions and reconciliation;
- stable errors and separate public/private readiness.

SDKs preserve these guarantees:

1. Unsupported order types are never silently converted.
2. Account/subaccount identity survives placement, cancellation, retrieval and reconciliation.
3. Incomplete or non-authoritative pages are not exposed as authoritative empty results.
4. Ambiguous placement remains `UNKNOWN` until reconciled.
5. Cancel-all exposes unresolved orders and completion state.
6. Canonical financial values remain decimal strings.
7. Streaming exposes reconnect, gap and freshness information when available.

## Belief Stream conformance

The SDKs consume `CORE-ADR-004`, `CLOUD-ADR-003` and `PREDICTION_PROVIDERS.md`.

Parity covers:

- `BeliefStreamRef` and immutable `PredictionRevision`;
- active, abstained, withdrawn and invalidated states;
- decimal-string probability, confidence, impact and scores;
- authoritative sequence, receipt time, content hash and source plane;
- corrections and supersession;
- structured drivers, risks, sources and bounded extensions;
- current state, freshness and derived metrics;
- synchronization cursors, completeness and sequence gaps;
- private local publication and commercial Cloud synchronization;
- entitlement, grace and revocation metadata;
- versioned Prediction and Belief Stream events.

SDKs preserve these guarantees:

1. Accepted revisions are immutable.
2. Corrections, abstentions, withdrawals and invalidations are new records or ledger-linked events.
3. Gap-affected state is not represented as complete or authoritative.
4. Duplicate IDs with conflicting hashes are integrity errors.
5. Missing/stale data is never converted to zero probability or confidence.
6. Core and Cloud sequence authority remains visible.
7. Text explanations are never parsed by helpers into executable values.

## Runner conformance

The SDKs consume `CORE-ADR-005`, `CLOUD-ADR-004` and `RUNNER_PROTOCOL.md`.

Parity covers:

- `RunnerManifest`, capabilities and resource limits;
- `StrategyExecutionPackage` and immutable hashes;
- `RunnerDeployment`, desired state and monotonic generation;
- target-specific logical connections;
- `RunnerLease`, expiry and permitted safety actions;
- deployment states including `ORPHANED`;
- idempotent commands and command status;
- ordered at-least-once events, replay cursors and gaps;
- checkpoints, integrity and compatibility;
- health, heartbeat and readiness;
- migration, source fencing and target restore;
- Cloud admission, quotas, usage and maintenance metadata.

SDKs preserve these guarantees:

1. Packages and checkpoints never contain resolved secrets.
2. Stale generations remain fenced/rejected and are not hidden by retry.
3. Lease loss is visible; helpers do not continue risk-increasing execution automatically.
4. A command timeout or missing acknowledgement is not success.
5. Event gaps require replay or reconciliation before state is authoritative.
6. Incompatible checkpoint restore is rejected.
7. Migration never starts two risk-increasing generations optimistically.
8. Process health, deployment liveness, lease ownership and Provider readiness remain separate.

## MCP conformance

The SDKs consume `CORE-ADR-006`, `CLOUD-ADR-005` and `MCP_PROTOCOL.md`.

Parity covers:

- `McpToolDescriptor`, catalog identity and owner plane;
- input/output schemas, precise scopes and effect classes;
- approval, idempotency, relay and data-classification metadata;
- structured calls, results, errors and warnings;
- approval challenges bound to exact semantic payloads;
- asynchronous operation handles and polling;
- Core/Cloud credential and catalog separation;
- OAuth protected-resource metadata for Cloud;
- relay grants, requests, responses, expiry and replay protection;
- private admin MCP exclusion from public clients.

SDKs preserve these guarantees:

1. Catalog filtering is not treated as authorization.
2. Financial/destructive calls preserve approval and idempotency identities.
3. A changed payload cannot reuse an approval challenge.
4. Timeout or ambiguous result is not converted to success or retried as a new mutation.
5. Cloud relay is not exposed as an arbitrary Core HTTP/tool proxy.
6. Core offline state is explicit; mutating relay calls are not queued for later.
7. Core denial, approval and structured errors survive relay translation.
8. Cloud never presents its credential as a Core credential.
9. Audit and telemetry redact arguments, secrets and sensitive results by default.

## Marketplace conformance

The SDKs consume `CORE-ADR-007`, `CLOUD-ADR-006` and `MARKETPLACE_PROTOCOL.md`.

Parity covers:

- `MarketplaceAsset` and asset kind;
- immutable `AssetRelease`, artifact and manifest hashes;
- compatibility, permissions, dependencies and attestations;
- mutable `MarketplaceListing` and versioned `MarketplaceOffer`;
- free, one-time and subscription pricing;
- `MarketplaceAcquisitionIntent` and `MarketplaceAcquisition`;
- `MarketplaceEntitlement` and signed entitlement receipts;
- short-lived `MarketplaceDistributionGrant`;
- `MarketplaceInstallRequest` and `AssetInstallationReceipt`;
- installation state, update availability, local modifications and quarantine;
- reviews, revocations and settlement summaries;
- publisher, entitlement and payout status operations owned by Cloud.

SDKs preserve these behavioral guarantees:

1. A listing is not an executable artifact; runtime authority comes from an immutable release.
2. The same release ID/content hash cannot represent different bytes.
3. Acquisition, entitlement and local installation remain distinct objects and operations.
4. A completed paid acquisition requires authoritative payment confirmation; `PENDING` or `PAYMENT_AMBIGUOUS` is never represented as success.
5. Amounts, fees, tax, publisher net and settlement values remain decimal strings.
6. Stale offer versions, price/currency mismatches and idempotency conflicts remain explicit errors.
7. Entitlement states (`ACTIVE`, `GRACE`, `EXPIRED`, `REVOKED`, `REFUNDED`, `CHARGEBACK`) remain visible and are not collapsed into booleans.
8. Distribution grants remain release-, hash-, audience-, target- and expiry-bound. They are never exposed as general Cloud credentials.
9. Core installation verifies hash, signature, compatibility, dependencies, permissions and entitlement; SDK helpers cannot bypass validation.
10. Permission escalation on update requires explicit approval.
11. Local modifications are preserved and are not silently overwritten by Marketplace updates.
12. Ordinary delisting is distinct from critical security revocation and quarantine.
13. Installable assets never contain raw secrets or unrestricted credentials.
14. Arbitrary executable plugins are rejected until a later sandbox protocol explicitly permits them.
15. Mutable listing counters are never represented as financial ledger authority or payout confirmation.
16. Publisher-facing APIs do not expose subscriber wallets, positions, local strategy configuration or Runner placement by default.

Core client operations include:

- inspect release manifests and compatibility;
- validate or stage a release;
- install, inspect, pin, update, rollback, quarantine and uninstall;
- inspect installation receipts, local lineage and entitlement receipt state;
- compare installed/local modifications with a newer release.

Cloud client operations include:

- browse assets/listings/releases and version-aware reviews;
- manage publisher assets, releases, offers and moderation status;
- acquire offers and inspect authoritative payment/acquisition status;
- list entitlements, renewals, grace, refunds and revocations;
- issue/consume distribution grants through target-specific flows;
- inspect publisher balances, settlement summaries and payout status;
- manage privacy-minimized installation claims and limits.

The SDKs do not expose raw payment-provider credentials, publisher payout secrets or unrestricted ledger-write methods.

## Language-specific freedom

Implementations may differ in:

- `Promise`/future/coroutine types;
- async iterator and stream APIs;
- error wrapper types;
- package/module layout;
- idiomatic naming;
- transport details;
- builder and option patterns.

Differences must not alter observable deadlines, cursor state, completeness, ambiguity, approval, entitlement, integrity, sequence, generation or fencing behavior.

## Security rules

- Raw credentials are never serialized into strategies, Provider manifests, execution packages, checkpoints, Marketplace releases or installation receipts.
- Core and Cloud tokens are not interchangeable.
- Convenience APIs cannot silently forward local data to Cloud.
- Venue and Marketplace connection bindings are opaque references; resolved secrets are not returned.
- Publisher ingestion and payout credentials are exposed only through narrowly scoped Cloud administration surfaces.
- Distribution grants and entitlement receipts are not reusable API credentials.
- SDK telemetry excludes prediction payloads, strategies, wallets, positions, packages, checkpoints, Marketplace asset bytes, installation details and secrets by default.
- Artifact downloads enforce bounded size and caller-controlled destinations; SDKs must not auto-execute downloaded content.

## Versioning

SDK package versions may advance independently, but each release declares:

- supported Core API/protocol versions;
- supported Cloud API/protocol versions;
- Provider and Venue Provider versions;
- Prediction Provider/Belief Stream specification versions;
- Runner protocol/runtime versions;
- MCP protocol versions;
- Marketplace protocol versions and supported asset kinds;
- event schema versions;
- temporary parity gaps.

Breaking canonical changes require a compatible SDK major-version strategy or explicit negotiated protocol transition.

An older SDK must reject an unsupported Marketplace asset kind or protocol rather than applying a lossy fallback.

## Conformance

Shared fixtures should cover:

- Venue order/account/pagination/reconciliation behavior;
- Prediction revision, decimal, correction, gap and entitlement behavior;
- Runner package, generation, lease, command, event, checkpoint and migration behavior;
- MCP scopes, effects, approval, idempotency, operations and relay behavior;
- immutable Marketplace releases and hash/signature verification;
- free, one-time and subscription offers;
- stale offer, price mismatch, acquisition idempotency and payment ambiguity;
- entitlement renewal, grace, expiry, refund and revocation;
- distribution grant expiry, replay and target mismatch;
- install compatibility, dependency conflict and permission escalation;
- local modification preservation, rollback and active-deployment uninstall protection;
- revocation/quarantine and verified-acquirer review rules;
- decimal settlement allocation and payout reconciliation status;
- privacy and redaction cases;
- compatibility matrices for Core, Cloud and all normative protocols.

CI should detect drift between canonical schemas and language implementations.

## Related decisions

- `CORE-ADR-001` and `CLOUD-ADR-001` define product boundaries.
- `CORE-ADR-002` and `CLOUD-ADR-002` define Provider runtime and commercial services.
- `CORE-ADR-003` defines Venue Provider semantics.
- `CORE-ADR-004` and `CLOUD-ADR-003` define Belief Streams.
- `CORE-ADR-005` and `CLOUD-ADR-004` define Runner runtime and Cloud control plane.
- `CORE-ADR-006` and `CLOUD-ADR-005` define MCP and remote relay.
- `CORE-ADR-007` and `CLOUD-ADR-006` define Marketplace installation, commerce, entitlements and settlement.
