# SDK-ADR-001 — SDK Architecture and Parity

- **Status:** Accepted
- **Date:** 2026-08-04
- **Scope:** `polyforge-sdk-ts`, `polyforge-sdk-python`, `polyforge-sdk-rust`
- **Shared revision:** 9

## Shared-decision rule

This ADR is shared by the three SDK repositories and remains logically identical in each implementation repository.

A shared change is accepted only when:

1. the shared revision is incremented;
2. equivalent changes are prepared for all three repositories;
3. implementation and conformance impact is recorded;
4. temporary parity exceptions are explicit and time bounded.

Language-specific decisions use `TS-ADR-*`, `PY-ADR-*` and `RS-ADR-*`.

## Principle

The TypeScript, Python and Rust SDKs expose equivalent public capabilities, validation, error meaning, authorization boundaries and lifecycle semantics.

Language-native ergonomics may differ, but an operation must not become safer, more authoritative or more successful merely because it is called from one language rather than another.

## Explicit API targets

Every SDK provides explicit equivalents of:

```text
PolyforgeCoreClient
PolyforgeCloudClient
```

Core and Cloud have distinct base URLs, credentials, scopes, availability and data-ownership assumptions.

An optional facade may coordinate both clients but must preserve:

- explicit target selection;
- separate authentication;
- explicit data movement;
- independent retries and failures;
- distinct permission checks;
- target-specific custody and availability semantics.

Cloud credentials are never treated as Core credentials, and local data is never forwarded to Cloud silently.

## Core client scope

Core clients cover self-hosted/runtime capabilities including:

- markets and Venue Provider capabilities;
- strategies, portable `.polyforge` resources and local Marketplace installations;
- backtests, paper trading and live execution;
- private Prediction Providers and cached authorized Belief Streams;
- execution packages, local/remote Runners, deployments, events and checkpoints;
- local MCP catalog, approvals and operations;
- local connections, secret lifecycle metadata and typed signer operations;
- deployment connection bindings and readiness;
- local backup policies, backup creation, restore plans and validation;
- release discovery, local preflight, upgrade operations, migrations and rollback;
- orders, positions, portfolio, risk, webhooks, audit and installation administration.

## Cloud client scope

Cloud clients cover operated-service capabilities including:

- accounts, workspaces and installation identity;
- Marketplace publishers, assets, releases, offers, acquisitions and entitlements;
- commercial Provider publication and Belief Stream services;
- Cloud Runner regions, classes, deployment, usage and migration;
- Cloud MCP, OAuth metadata and remote-relay administration;
- optional managed connections, custody modes, vault/remote-signer/federation integration;
- managed backup plans, retention, restore orchestration and DR testing;
- signed release catalogue, channels, compatibility, managed-update policy and rollout status;
- notifications, monitoring, billing, quotas and settlement references.

Public SDK clients do not expose private admin MCP or generic secret-export functionality by default.

## Canonical contract ownership

SDKs consume canonical contracts owned by Core and Cloud:

- `PREDICTION_PROVIDERS.md`;
- `RUNNER_PROTOCOL.md`;
- `MCP_PROTOCOL.md`;
- `MARKETPLACE_PROTOCOL.md`;
- `CONNECTIONS_AND_SIGNING_PROTOCOL.md`;
- `BACKUP_AND_RESTORE_PROTOCOL.md`;
- `UPDATE_AND_MIGRATION_PROTOCOL.md`.

Public types should be generated from or validated against versioned OpenAPI, JSON Schema and event fixtures where practical.

SDKs must not invent language-only public models that conflict with canonical contracts.

## General parity requirements

A capability is complete only when:

1. its canonical contract is versioned;
2. all three SDKs implement it or an explicit parity exception exists;
3. shared conformance fixtures pass;
4. supported Core/Cloud/protocol versions are declared;
5. pagination, streaming, cancellation, idempotency, approval, ambiguity, sequence, freshness, generation and fencing remain visible.

Structured fields are authoritative. Human-readable messages are supplemental and must not be parsed to recover missing domain state.

## Venue Provider conformance

The SDKs preserve `CORE-ADR-003` semantics for:

- `VenueId`, `VenueMarketRef` and `VenueAccountRef`;
- account, wallet and subaccount continuity;
- typed capabilities and manifests;
- authoritative/completeness metadata and pagination;
- ordered market-data snapshots/deltas, freshness and gaps;
- exact order type, time-in-force and expiration;
- accepted, rejected and `UNKNOWN` placement outcomes;
- cancellation, cancel-all, fills, fees, positions and reconciliation;
- decimal-string financial values and stable errors.

Unsupported order semantics are never silently downgraded. Incomplete data is never exposed as an authoritative empty result. Ambiguous placement remains `UNKNOWN` until reconciled.

## Belief Stream conformance

The SDKs preserve `CORE-ADR-004`, `CLOUD-ADR-003` and `PREDICTION_PROVIDERS.md` semantics for:

- immutable `PredictionRevision` history;
- `BeliefStreamRef`, state, sequence and freshness;
- active, abstained, withdrawn and invalidated states;
- canonical decimal-string probability/confidence;
- corrections and supersession;
- duplicate-ID/hash conflicts;
- sequence gaps, cursors and resynchronization;
- local private publication and Cloud commercial synchronization;
- entitlement, grace and revocation metadata;
- versioned events and structured drivers/risks/sources.

Missing, stale, expired or gap-affected predictions are never converted to zero probability or confidence.

## Runner conformance

The SDKs preserve `CORE-ADR-005`, `CLOUD-ADR-004` and `RUNNER_PROTOCOL.md` semantics for:

- Runner manifests, capabilities and limits;
- immutable execution packages and content hashes;
- deployment desired state and monotonic generation;
- leases, expiry and explicitly permitted safety actions;
- idempotent commands and durable command state;
- ordered at-least-once events, deduplication and replay gaps;
- checkpoints, compatibility and restore;
- process health, deployment liveness, lease ownership and Provider readiness as distinct facets;
- source fencing and target restore during migration;
- Cloud admission, quotas, maintenance and usage.

A timeout is not command success. A stale generation is not retried as a fresh action. Lease loss remains visible and cannot be hidden by automatic restart.

## MCP conformance

The SDKs preserve `CORE-ADR-006`, `CLOUD-ADR-005` and `MCP_PROTOCOL.md` semantics for:

- separate Core, Cloud and private Admin planes;
- tool descriptors, versions, scopes, effects and data classification;
- exact input/output schemas;
- approval challenges bound to actor, payload hash, target, expiry and idempotency key;
- asynchronous operation handles;
- stable structured errors;
- explicit relay grants, offline behavior, replay protection and Core reauthorization;
- minimal redacted audit metadata.

SDK helpers must not expose arbitrary Core API relay, arbitrary-sign, secret-read or generic credential-injecting HTTP tools.

## Marketplace conformance

The SDKs preserve `CORE-ADR-007`, `CLOUD-ADR-006` and `MARKETPLACE_PROTOCOL.md` semantics for:

- stable assets and immutable content-addressed releases;
- mutable listings separated from versioned offers;
- acquisition/payment states separated from entitlements;
- distribution grants separated from local installation receipts;
- compatibility, permissions, dependencies and attestations;
- local pinning, update, rollback, provenance and modifications;
- reviews tied to verified acquisition/version context;
- revocation, quarantine and settlement references;
- decimal-string price, fee, tax and payout amounts.

A purchase is not represented as a successful Core installation. Payment ambiguity remains explicit. Mutable listing counters are never treated as financial authority.

## Connections and signer conformance

The SDKs preserve `CORE-ADR-008`, `CLOUD-ADR-007` and `CONNECTIONS_AND_SIGNING_PROTOCOL.md` semantics for:

- opaque `ConnectionRef` and redacted `ConnectionMetadata`;
- custody modes and external secret backends;
- immutable secret-bundle versions;
- protected import flows that never return plaintext secrets;
- connection use policies and permission summaries;
- deployment/generation-bound connection bindings;
- typed payload-bound signer operations and immutable receipts;
- signer-mediated calls, remote signers, user-managed vaults and ephemeral federation;
- multidimensional readiness;
- rotation, revocation, deletion and audit receipts.

Possession of a connection ID is not authorization. SDKs never offer generic private-key export, generic arbitrary-sign or silent local-to-Cloud secret copying.

## Backup and Restore conformance

The SDKs consume `CORE-ADR-009`, `CLOUD-ADR-008` and `BACKUP_AND_RESTORE_PROTOCOL.md`.

At minimum, parity covers:

- `BackupManifest`, `BackupComponent` and content hashes;
- authority classes: authoritative, append-only, checkpointed, cache, derived, external reference and encrypted secret;
- application-, database-, crash-consistent and resource-export backups;
- encryption modes and key dependencies;
- policies, schedules, RPO/RTO targets and retention tiers;
- durable create/upload/verify/restore-test operations;
- restore plans, preflight, identity conflicts and secret policy;
- same-installation, replacement-installation, isolated-test and partial-import modes;
- validation reports, Venue/Belief Stream reconciliation and derived-state rebuild;
- deletion receipts and retained-media state.

The SDKs preserve these guarantees:

1. Upload completion is not restore verification.
2. Cache or derived components are not presented as authoritative after restore.
3. Plaintext secrets never appear in backup models or logs.
4. Missing key dependencies produce locked/incomplete state, not empty credentials.
5. Restored Runner leases, generations and grants are never active.
6. Revoked/deleted secrets are not reactivated silently from an old backup.
7. A restore is not `COMMITTED` before mandatory validation succeeds.
8. `safeForLiveExecution` remains false until Venue, connection and runtime reconciliation pass.
9. Deletion distinguishes catalogue deletion, object deletion, crypto-erasure and retention pending.
10. Target RPO/RTO is not represented as observed achievement.

Core operations include backup policy management, local snapshot creation, export, preflight, restore, validation and retention.

Cloud operations include managed plan enrollment, upload grants, storage/replication state, restore orchestration, restore tests, quotas and deletion.

## Update and Migration conformance

The SDKs consume `CORE-ADR-010`, `CLOUD-ADR-009` and `UPDATE_AND_MIGRATION_PROTOCOL.md`.

At minimum, parity covers:

- immutable `ReleaseRef`, `ReleaseManifest` and artifact digests;
- build provenance, signatures and release channels;
- compatibility matrices and support windows;
- preflight requirements, backup requirements and maintenance windows;
- typed immutable migration steps, dependencies and validation checks;
- upgrade plans, approvals and durable operation state;
- post-upgrade validation and reconciliation reports;
- rollback classes and plans;
- managed-update policy, update grants and rollout rings;
- signed release revocation and security-advisory metadata.

The SDKs preserve these guarantees:

1. Mutable tags/channel pointers are not artifact authority.
2. Invalid signatures or digest mismatches are hard failures.
3. Schema drift and migration checksum conflict remain explicit blockers.
4. Backup requirements cannot be silently bypassed by convenience helpers.
5. Active Runner drain/fencing requirements remain visible.
6. Upgrade timeout or client disconnect is not failure or success; the operation is queried by ID.
7. Irreversible migration and no-downgrade boundaries require explicit approval metadata.
8. A heartbeat is not post-upgrade validation.
9. Rollback class remains explicit; `FORWARD_FIX_ONLY`, `RESTORE_REQUIRED` and `NO_DOWNGRADE` are not represented as reversible.
10. Rollback does not claim to reverse external Venue, payment or settlement side effects.
11. Cloud managed-update grants cannot execute arbitrary commands.
12. Self-hosted update timing remains under local policy.

Core operations include release verification, compatibility/preflight, upgrade execution, migration progress, validation, rollback and local policy.

Cloud operations include release/channel discovery, signed manifest distribution, update enrollment, grants, rollout status, support windows and revocations.

## Language-specific freedom

Implementations may differ in:

- async primitives;
- iterator/stream APIs;
- typed error wrappers;
- package/module layout;
- cancellation tokens;
- language naming conventions;
- transport internals.

They must preserve the same observable states, deadlines, ambiguity, cursors, sequences, approvals, hashes, generations, custody, restore and rollback semantics.

## Security rules

- Core and Cloud credentials are not interchangeable.
- Raw credentials are never serialized into strategies, packages, checkpoints, backups, release manifests or ordinary SDK responses.
- Secret import values are redacted from logs, traces and exceptions.
- Sensitive errors never expose upstream bodies, private object URLs, ciphertext fields, tokens, keys or signatures when replay-sensitive.
- Backup and update telemetry excludes private strategies, wallets, orders, positions and secrets by default.
- SDK helpers cannot silently forward local data or secrets to Cloud.
- Public clients do not expose private admin MCP or unrestricted operator access.

## Versioning

Each SDK release declares supported:

- Core and Cloud API versions;
- Provider, Venue and Belief Stream protocols;
- Runner, MCP and Marketplace protocols;
- Connections/Signing protocol;
- Backup/Restore protocol;
- Update/Migration protocol;
- event schema versions;
- known temporary parity gaps.

Breaking canonical changes require a compatible major-version strategy or an explicit negotiated transition.

## Conformance

Shared fixtures should cover all prior domain contracts plus:

### Backup/Restore

- complete, incomplete and corrupt manifests;
- encrypted-secret inclusion and missing key dependencies;
- stale Runner and revoked-secret resurrection prevention;
- replacement-installation identity rebinding;
- Venue and Belief Stream reconciliation;
- partial restore conflicts;
- restore testing, retention and deletion receipts.

### Update/Migration

- signed and invalid manifests;
- artifact digest mismatch and release revocation;
- supported and skipped upgrade paths;
- schema drift and migration hash conflicts;
- backup and maintenance preflight blockers;
- Runner drain/fencing;
- idempotent/resumable migration steps;
- rollback classes and restore-assisted recovery;
- post-upgrade validation and rollout halt behavior.

CI should detect drift between published schemas and all language implementations.

## Related decisions

- `CORE-ADR-003` — Venue Providers.
- `CORE-ADR-004` / `CLOUD-ADR-003` — Belief Streams.
- `CORE-ADR-005` / `CLOUD-ADR-004` — Runners.
- `CORE-ADR-006` / `CLOUD-ADR-005` — MCP and relay.
- `CORE-ADR-007` / `CLOUD-ADR-006` — Marketplace.
- `CORE-ADR-008` / `CLOUD-ADR-007` — Connections and signer custody.
- `CORE-ADR-009` / `CLOUD-ADR-008` — Backup, restore and DR.
- `CORE-ADR-010` / `CLOUD-ADR-009` — Releases, upgrades and managed updates.
