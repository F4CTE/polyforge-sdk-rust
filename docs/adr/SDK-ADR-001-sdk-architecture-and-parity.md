# SDK-ADR-001 — SDK Architecture and Parity

- **Status:** Accepted
- **Date:** 2026-08-03
- **Scope:** `polyforge-sdk-ts`, `polyforge-sdk-python`, `polyforge-sdk-rust`
- **Shared revision:** 1

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

Every SDK supports two distinct targets:

### Polyforge Core

Core clients cover runtime and self-hosted capabilities, including:

- markets and venue capabilities;
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

## Parity requirements

A public capability is complete only when:

1. its canonical contract is versioned;
2. all three SDKs implement it, or a temporary parity exception is recorded;
3. all SDKs pass the same conformance examples and fixtures;
4. Core/Cloud compatibility requirements are documented;
5. streaming, pagination, cancellation and idempotency semantics are represented consistently.

## Language-specific freedom

Implementations may differ in:

- async primitives;
- iterator and streaming APIs;
- error wrapper types;
- package/module layout;
- naming adaptations required by language conventions;
- transport implementation details.

These differences must not alter observable contract behavior.

## Security rules

- Raw credentials are never serialized into strategies, Provider manifests or Runner packages.
- Core and Cloud tokens are not interchangeable.
- Convenience APIs cannot silently forward local data to Cloud.
- Remote MCP relay operations remain explicit paid Cloud operations.
- Sensitive values must be redacted from errors and logs consistently across SDKs.

## Versioning

SDK package versions may advance independently, but each release declares:

- supported Core API/protocol versions;
- supported Cloud API/protocol versions;
- Provider protocol versions;
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
- portable strategy examples;
- compatibility matrices.

CI should detect drift between published schemas and language implementations.

## Related decisions

- `CORE-ADR-001` and `CLOUD-ADR-001` define product boundaries.
- `CORE-ADR-002` defines Provider runtime contracts.
- `CLOUD-ADR-002` defines commercial Provider services.
