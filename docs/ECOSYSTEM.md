# Polyforge ecosystem

> **Own your infrastructure. Rent convenience.**
>
> **Dark by default. Precise by design.**

Polyforge has five active repositories:

1. `F4CTE/PolyForge-core` — self-hosted product and reference runtime.
2. `F4CTE/PolyForge` — Cloud control plane and managed services.
3. `F4CTE/polyforge-sdk-ts` — TypeScript SDK.
4. `F4CTE/polyforge-sdk-python` — Python SDK.
5. `F4CTE/polyforge-sdk-rust` — Rust SDK.

The archived `F4CTE/polyforge-mcp` repository is discontinued. Core and Cloud each embed the MCP server for the capabilities they own.

## SDK contract

This SDK must remain functionally equivalent to the TypeScript and Python SDKs. It exposes explicit clients for:

- Polyforge Core: markets, strategies, backtests, orders, portfolio, providers, whale tracking, copy trading, runners and local runtime administration;
- Polyforge Cloud: accounts, installations, marketplace, subscriptions, Cloud Runners, backups, notifications and optional remote MCP relay management.

The SDK does not merge Core and Cloud authorization boundaries. Unified convenience APIs may orchestrate both clients, but the target and required permissions must remain explicit.

Local Core MCP and API access remain free. Cloud-operated relay, runner, backup and marketplace services may require a subscription.
