# SDK-ADR-001 — SDK Architecture and Parity

- **Status:** Accepted
- **Date:** 2026-08-04
- **Scope:** `polyforge-sdk-ts`, `polyforge-sdk-python`, `polyforge-sdk-rust`
- **Shared revision:** 7

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

Language-native ergonomics may differ, but resource coverage, validation, events, error meaning, security boundaries and authorization behavior remain equivalent.

## Explicit API targets

Every SDK supports two distinct targets.

### Polyforge Core

Core clients cover self-hosted and runtime capabilities, including:

- markets and Venue Provider capabilities;
- strategies, portable `.polyforge` packages and local Marketplace installations;
- backtests, paper trading and live execution;
- local/private Prediction Providers and authorized cached Belief Streams;
- execution-package compilation and local/user-operated Runners;
- Runner deployments, commands, health, events, checkpoints and migration;
- local MCP catalog, credentials, approvals and operations;
- local connections, secret lifecycle metadata and typed signer operations;
- deployment connection bindings and readiness;
- orders, positions, portfolio and risk;
- local webhooks, audit and installation administration.

### Polyforge Cloud

Cloud clients cover operated-service capabilities, including:

- accounts, workspaces and installation identity;
- Marketplace assets, releases, offers, acquisitions and entitlements;
- commercial Provider publication and Belief Stream services;
- Cloud Runner regions, classes, admission, deployment, usage and migration;
- Cloud MCP, OAuth metadata and remote relay administration;
- optional managed connections and custody-mode provisioning;
- user-managed vault, remote signer and federation integrations;
- managed rotation, revocation, deletion and audit summaries;
- backups, notifications, monitoring, quotas, billing and settlement references.

Public SDK clients do not expose private admin MCP tools or generic secret-export operations by default.

Core and Cloud have distinct base URLs, credentials, permissions and availability assumptions. A convenience `PolyforgeClient` may coordinate both but must not hide the target, merge credentials or silently move private data between planes.

## Canonical contract ownership

- Core runtime contracts are owned by `F4CTE/PolyForge-core` ADRs, schemas and APIs.
- Cloud commercial/control-plane contracts are owned by `F4CTE/PolyForge` ADRs, schemas and APIs.
- `PREDICTION_PROVIDERS.md` is the normative Prediction Provider and Belief Stream contract.
- `RUNNER_PROTOCOL.md` is the normative Runner contract.
- `MCP_PROTOCOL.md` is the normative MCP descriptor, result, approval, operation and relay contract.
- `MARKETPLACE_PROTOCOL.md` is the normative Marketplace asset, entitlement and installation contract.
- `CONNECTIONS_AND_SIGNING_PROTOCOL.md` is the normative connection, custody, secret lifecycle, binding and signer-operation contract.
- SDKs consume generated or validated forms of canonical contracts and must not invent conflicting language-only domain models.

Public types should be generated from or validated against versioned OpenAPI, JSON Schema and event-schema fixtures where practical.

## Client shape

Each SDK provides explicit equivalents of:

```text
PolyforgeCoreClient
PolyforgeCloudClient
```

An optional higher-level facade preserves:

- explicit target selection;
- separate authentication;
- explicit data movement;
- independent retries and failures;
- distinct permission checks;
- target-specific connection and custody semantics.

## General parity requirements

A public capability is complete only when:

1. its canonical contract is versioned;
2. all three SDKs implement it, or a temporary parity exception is recorded;
3. all SDKs pass shared conformance fixtures;
4. Core/Cloud compatibility requirements are documented;
5. pagination, streaming, cancellation, idempotency, approval, sequence, freshness, generation, fencing, entitlement, custody and secret-lifecycle semantics remain explicit.

## Venue Provider conformance

The SDKs consume `CORE-ADR-003` and preserve:

- extensible `VenueId`;
- `VenueMarketRef` and canonical-market mappings;
- `VenueAccountRef`, wallet, account and subaccount identity;
- typed manifests and capabilities;
- cursor pages with completeness and authority metadata;
- ordered market-data snapshots and deltas;
- freshness and sequence gaps;
- `VenueOrderIntent` with decimal-string financial values;
- accepted, rejected and unknown placement outcomes;
- scoped cancellation and authoritative reconciliation;
- separate public-data and private-execution readiness.

Unsupported order types are never silently converted. Ambiguous placement remains `UNKNOWN`. Account identity is preserved across placement, cancellation, retrieval and reconciliation.

## Belief Stream conformance

The SDKs consume `CORE-ADR-004`, `CLOUD-ADR-003` and `PREDICTION_PROVIDERS.md`.

Parity covers:

- `BeliefStreamRef` and immutable `PredictionRevision`;
- active, abstained, withdrawn and invalidated states;
- decimal-string probability and confidence;
- sequence, authoritative receipt time, hashes and source plane;
- corrections and supersession;
- structured drivers, risks and sources;
- freshness, expiry, gaps and synchronization cursors;
- private local publication and Cloud commercial distribution;
- entitlement and revocation state;
- versioned events and immutable history.

Missing, stale, expired or withdrawn predictions are never converted to zero probability or confidence.

## Runner conformance

The SDKs consume `CORE-ADR-005`, `CLOUD-ADR-004` and `RUNNER_PROTOCOL.md`.

Parity covers:

- `RunnerManifest` and compatibility;
- `StrategyExecutionPackage` and immutable hashes;
- `RunnerDeployment`, monotonic generations and desired state;
- deployment connection bindings;
- `RunnerLease`, expiry and permitted safety actions;
- idempotent commands and command status;
- ordered at-least-once events and replay gaps;
- checkpoints, integrity and restore compatibility;
- migration with source fencing and target reconciliation;
- Cloud admission, quotas, metering and maintenance.

A stale generation is never hidden by retry. Lease loss remains visible. A timeout is not success. A target is not started before the source is fenced during migration.

## MCP conformance

The SDKs consume `CORE-ADR-006`, `CLOUD-ADR-005` and `MCP_PROTOCOL.md`.

Parity covers:

- separate Core, Cloud and private Admin planes;
- `McpToolDescriptor`, effects, scopes and relay policy;
- structured calls, results, warnings and stable errors;
- payload-bound approvals and idempotency;
- asynchronous operation handles;
- OAuth protected-resource metadata for Cloud;
- explicit relay grants, signed requests and Core reauthorization;
- offline behavior, replay protection and audit redaction.

SDK helpers must not expose a generic arbitrary Core relay or arbitrary HTTP proxy. A failed or timed-out financial MCP call is not automatically retried with a fresh idempotency identity.

## Marketplace conformance

The SDKs consume `CORE-ADR-007`, `CLOUD-ADR-006` and `MARKETPLACE_PROTOCOL.md`.

Parity covers:

- stable assets and immutable content-addressed releases;
- mutable listings and versioned offers;
- acquisition and payment states, including ambiguous payment;
- entitlements, rights, expiry, grace and revocation;
- short-lived distribution grants;
- local installation requests and immutable installation receipts;
- compatibility, permissions, dependencies and safety attestations;
- update, pinning, rollback, local modifications and quarantine;
- verified-acquirer reviews;
- decimal-string amounts and publisher settlement references.

A purchase is not represented as a local installation. Paid acquisition is not successful until payment authority is confirmed. Release hashes and bytes are immutable.

## Connections and signer conformance

The SDKs consume `CORE-ADR-008`, `CLOUD-ADR-007` and `CONNECTIONS_AND_SIGNING_PROTOCOL.md`.

At minimum, parity covers:

- `ConnectionRef` and redacted `ConnectionMetadata`;
- custody modes: local managed, Cloud managed, user-managed vault, remote signer, ephemeral federation and public no-secret;
- `SecretBundleRef` and versioned lifecycle states;
- protected secret import requests without secret-bearing responses;
- `ConnectionUsePolicy`, actors, permissions and constraints;
- deployment connection bindings with generation scope;
- `SignerOperationRequest`, exact payload hashes, target binding and deadlines;
- structured signer results and immutable receipts;
- mediated-operation policies and endpoint classes;
- multidimensional connection readiness;
- rotation, revocation and deletion requests/results;
- managed provisioning, ephemeral grants and remote signer envelopes;
- audit records, canary events and stable redacted errors.

The SDKs preserve these guarantees:

1. A `ConnectionRef` never contains secret material.
2. Possession of a connection ID does not imply authorization.
3. Ordinary SDK calls never return private keys, API secrets, refresh tokens, encrypted blobs or reusable unrestricted headers.
4. Secret import values are accepted only by protected operations and are redacted from errors, logs and representations.
5. Secret versions are immutable references; rotation creates a new version rather than overwriting the active value silently.
6. The same request/operation identity with a different payload is an idempotency conflict.
7. Signer approvals bind actor, connection, exact payload hash, target, deployment generation, expiry and idempotency identity.
8. Arbitrary-sign and arbitrary-URL credential-injection helpers are prohibited.
9. A stale Runner generation or expired lease cannot receive new risk-increasing signatures or credentials.
10. Remote signer denial, offline and timeout states remain explicit and are not downgraded to another custody mode.
11. Cloud-managed custody is distinct from user-managed vault and remote-signer modes; SDK terminology must not misrepresent custody.
12. Local Core connections are not copied to Cloud implicitly.
13. Revocation blocks new use and never silently switches dependent workloads to another connection.
14. Deletion receipts distinguish logical deletion, crypto-erasure, external-reference removal and backup-retention pending state.
15. Missing credentials are not represented as empty balances, empty orders or healthy private execution.
16. Audit models expose references and hashes, never raw secrets or replay-sensitive signed payloads.

### Core connection operations

Core clients may expose:

- list/get redacted connections;
- create metadata and initiate protected import/OAuth flows;
- inspect readiness, permissions and audit summaries;
- create/update connection-use policies;
- bind connections to strategies and Runner deployments;
- request typed signer or mediated operations;
- rotate, revoke and delete;
- inspect operation and reconciliation status;
- configure external vaults and local/remote signers.

### Cloud managed-connection operations

Cloud clients may expose:

- provision managed connections with explicit custody mode;
- configure user-managed vault, remote signer and federation integrations;
- inspect redacted health and workload bindings;
- bind connections to Cloud Runner deployments;
- initiate rotation, reauthorization, revocation and deletion;
- inspect JIT grants, audit summaries and deletion receipts;
- manage regional/custody capabilities and entitlements.

Neither client exposes a general decrypted-secret read API.

## Language-specific freedom

Implementations may differ in:

- async primitives;
- iterators and streams;
- error wrappers;
- package/module layout;
- language-conventional naming;
- secure input helpers;
- transport implementation.

Differences must not alter contract meaning.

Examples:

- TypeScript may use `Promise`, `AsyncIterable`, `AbortSignal` and secret-value wrapper types;
- Python may use async iterators, context managers and mutable byte arrays;
- Rust may use `Future`, `Stream`, typed errors, secret-zeroizing wrappers and explicit cancellation tokens.

All languages represent the same deadlines, ambiguity, idempotency, custody, secret version, policy, readiness, generation and deletion semantics.

## Security rules

- Raw credentials are never serialized into strategies, Marketplace releases, Provider manifests, Runner packages or ordinary checkpoints.
- Core and Cloud tokens are not interchangeable.
- Convenience clients cannot silently forward local secrets or private data to Cloud.
- Secret values and import payloads must be redacted from debug output and exception formatting.
- SDK telemetry excludes strategies, prediction payloads, wallets, positions, packages, checkpoints, secret metadata and signing payloads by default.
- Connection metadata may be sensitive even when it contains no secret and must respect authorization.
- Reusable signed headers or tokens are returned only to explicitly trusted operation contracts and must remain target/expiry bounded.
- Remote MCP relay never turns a Cloud credential into a Core credential.
- Marketplace assets cannot embed connection secrets.

## Versioning

SDK releases declare:

- supported Core and Cloud API/protocol versions;
- Provider and Venue contract versions;
- Prediction/Belief Stream specification version;
- Runner protocol/runtime versions;
- MCP protocol version;
- Marketplace protocol version;
- Connections and Signing protocol version;
- supported custody modes and signer operation versions;
- event schema versions;
- known temporary parity gaps.

Breaking canonical changes require a compatible SDK major-version strategy or explicit negotiated protocol transition.

## Conformance

Shared fixtures should include:

- Core/Cloud authentication separation;
- Provider and Venue manifests, account isolation and reconciliation;
- Belief Stream immutability, decimal precision, gaps and expiry;
- Runner generations, lease loss, command idempotency and checkpoint restore;
- MCP scopes, approvals, relay replay and asynchronous operations;
- Marketplace release hashes, payment ambiguity, entitlements, installation and quarantine;
- connection metadata redaction and identity continuity;
- secret import idempotency and serialization redaction;
- AAD/key-version failures and rotation states;
- policy denial and exact approval binding;
- deployment connection generation fencing;
- allowed/forbidden signer targets;
- mediated operation timeout and ambiguous result;
- remote signer offline/denied cases;
- ephemeral grant expiry and replay;
- revocation with explicit risk-reducing exception;
- deletion receipts and backup retention;
- canary secret access and audit redaction;
- compatibility matrices and temporary parity exceptions.

CI should detect drift between canonical schemas and language implementations.

## Related decisions

- `CORE-ADR-002` and `CORE-ADR-003` define Provider, Venue, account and signer boundaries.
- `CORE-ADR-004` and `CLOUD-ADR-003` define Belief Streams.
- `CORE-ADR-005` and `CLOUD-ADR-004` define Runners and deployment generations.
- `CORE-ADR-006` and `CLOUD-ADR-005` define MCP and relay.
- `CORE-ADR-007` and `CLOUD-ADR-006` define Marketplace commerce and installation.
- `CORE-ADR-008` defines local connections, secrets and signer isolation.
- `CLOUD-ADR-007` defines optional managed connections and Cloud custody.
