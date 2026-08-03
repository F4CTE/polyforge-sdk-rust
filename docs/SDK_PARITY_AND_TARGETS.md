# Polyforge SDK parity and targets

## Status

Adopted on 2026-08-03.

## Principle

The TypeScript, Python and Rust Polyforge SDKs expose the same public capabilities.

Language-specific ergonomics may differ, but supported resources, schemas, events, validation behavior and error semantics must remain equivalent.

## API targets

The SDK supports two explicit targets:

- **Polyforge Core** — self-hosted product and reference strategy runtime;
- **Polyforge Cloud** — accounts, marketplace, backups, Cloud Runners and commercial services.

A high-level client may expose both targets, but Core and Cloud authentication, base URLs and capabilities must remain distinguishable.

## Shared capability groups

Parity applies to public capabilities including:

- markets and venue capabilities;
- strategies and portable `.polyforge` packages;
- backtests, paper trading and live execution;
- runners and deployments;
- orders, positions, portfolio and risk;
- whale tracking and copy trading;
- Providers, predictions and Belief Streams;
- MCP, webhooks and events;
- Cloud accounts, installations, marketplace subscriptions, backups and Cloud Runner operations.

## Contract rule

Public types must be generated from or validated against canonical OpenAPI, JSON Schema and event contracts. This SDK must not invent a Rust-only domain contract.

Raw credentials must never be serialized into portable strategies or runner deployment packages.

## Parity changes

A public capability is complete only when:

1. the canonical contract is versioned;
2. TypeScript, Python and Rust SDK support is implemented or an explicit temporary parity gap is recorded;
3. conformance fixtures pass against the same examples;
4. Core and Cloud compatibility requirements are documented.
