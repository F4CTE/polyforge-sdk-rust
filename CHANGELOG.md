# Changelog

## [Unreleased]

### Security
- **Default URL from env var** — `new()` now reads `POLYFORGE_API_URL` env var before falling back to `https://localhost:3002`, preventing silent credential exposure when deployed without explicit URL configuration; extended `is_local` to cover `0.0.0.0`, `127.0.0.x` range, and `localhost.localdomain` (closes #71)
- **CI**: switch from self-hosted runner to `ubuntu-latest` for all events and add `permissions: contents: read` to restrict GITHUB_TOKEN scope — mitigates supply chain risk from external PRs running on self-hosted infra (closes #72)
- **Clippy CI gate enforced** — removed `continue-on-error: true` from Clippy step so lint warnings (including security-relevant ones) now block CI; fixed `map_or` → `is_some_and` clippy warning (closes #79)
- **README documents HTTPS default** — corrected `http://` to `https://` in API reference table to match the actual `DEFAULT_BASE_URL` in code (closes #78)
- **HTTP request timeouts** — added `timeout(30s)` and `connect_timeout(10s)` to `reqwest::Client::builder()` to prevent indefinite hangs on unresponsive servers (closes #24)
- **Disable automatic redirects** — set `redirect(Policy::none())` on the HTTP client to prevent the Bearer token from being forwarded to third-party hosts via 3xx redirects (closes #25)
- **Webhook secret skip_serializing** — added `#[serde(skip_serializing)]` to `Webhook.secret` field to prevent the HMAC signing secret from leaking via `serde_json::to_string()` or any serialization path; regression of #8 where only `Debug` was fixed but `Serialize` was missed (closes #43)

### Fixed
- **BREAKING** `PlaceSmartOrderParams`: revert `interval_seconds`/`"intervalSeconds"` back to `interval_minutes`/`"intervalMinutes"` — the #66 fix was based on incorrect platform contract info; platform DTO uses `intervalMinutes` (closes #80)
- **Issue #50**: `ai_query()` now sends `{ "question": ... }` instead of `{ "query": ... }` to match the platform's `AiQueryDto` contract — every call was previously rejected or silently ignored by the backend
- **Issue #46**: `TradingMode` enum now serializes to `"LIVE"` / `"PAPER"` (uppercase) instead of `"live"` / `"paper"` — `start_strategy()` was receiving a 422 from the backend due to failed enum validation
- **Issue #47**: `WebhookEvent` variants now serialize to dot-notation strings (`"order.filled"`, `"strategy.error"`, `"whale.trade"`, `"news.signal"`, `"backtest.complete"`, `"daily.loss.limit"`, `"market.resolved"`, `"price.alert"`) instead of `SCREAMING_SNAKE_CASE` — webhooks registered via the SDK were using invalid event type strings
- **Issue #44**: `create_strategy_from_description()` now sends `{ "query": ... }` instead of `{ "description": ... }` to match the platform's `from-description` endpoint contract — the AI pipeline was receiving no input and returning empty/default strategies
- **BREAKING** `handle_response()`: handle 204 No Content by returning `serde_json::Value::Null` instead of crashing on empty body — `delete_strategy()` now returns `Result<()>` (closes #70)
- **BREAKING** `PlaceSmartOrderParams`: rename `interval_minutes` / `"intervalMinutes"` to `interval_seconds` / `"intervalSeconds"` to match platform contract — TWAP/DCA orders were executing 60x too fast (closes #66)

## [1.5.0] — 2026-04-03

### Fixed
- **BREAKING**: `create_strategy_from_description()` now sends `marketId` (camelCase) instead of `market_id` to match the platform contract (closes #14)
- `PaginatedResponse` now deserializes `totalPages` and `hasNext` correctly via `#[serde(rename)]` annotations — pagination was silently broken (closes #12)
- Added `#[serde(rename_all = "camelCase")]` to `Strategy`, `Portfolio`, `Position`, `Order`, `TraderScore`, `WhaleTrade`, `NewsSignal`, `Alert`, `CopyConfig`, `Webhook` — all camelCase fields now deserialize correctly instead of defaulting to `None` (closes #13)
- SSRF bypass in `validate_webhook_url()` — expanded IPv6 checks to cover unique-local (`fc00::/7`), link-local (`fe80::/10`), IPv4-mapped (`::ffff:x.x.x.x`), unspecified addresses, and cloud metadata hostnames (closes #10)

## [1.4.3] — 2026-04-03

### Fixed
- `create_strategy_from_description`: send `"marketId"` instead of `"market_id"` in request body to match platform API (closes #14)

## [1.4.2] — 2026-04-03

### Fixed
- **Security**: `with_url()` now validates the base URL — rejects non-HTTPS schemes (HTTP is still allowed for `localhost`/`127.0.0.1` during development), malformed URLs, path-traversal sequences (`..`), and embedded query strings or fragments (closes #11)
- **Security**: `get_orders()` query parameter values are now URL-encoded via `urlencoding::encode()`, preventing query-parameter injection (closes #9)
- Add `#[serde(rename_all = "camelCase")]` to `PaginatedResponse` — `total_pages` and `has_next` now correctly deserialize from the API's camelCase JSON keys (closes #12)
- Add `#[serde(rename_all = "camelCase")]` to all core API response types (`Market`, `Strategy`, `Portfolio`, `Position`, `Order`, `TraderScore`, `WhaleTrade`, `NewsSignal`, `Alert`, `CopyConfig`, `Webhook`) — snake_case fields now correctly map to camelCase JSON returned by the platform (closes #13)

### Added
- `validate_base_url()` internal helper with comprehensive URL validation
- Unit tests for base URL validation (scheme, localhost exception, malformed, traversal, query, fragment)

## [1.4.1] — 2026-03-30

### Fixed
- `MarketSentiment` struct: renamed `label` → `direction` to match the actual API response field (`direction: String`, values `"BULLISH" | "BEARISH" | "NEUTRAL"`)

## [1.4.0] — 2026-03-30

### Added
- `get_accuracy()` — `GET /api/v1/accuracy/me`; returns `AccuracyScore` with Brier score, win rate, calibration buckets, and per-category breakdown
- `get_portfolio_review()` — `GET /api/v1/ai/portfolio-review`; returns `PortfolioReview` with AI-generated review text, suggestions list, and score (1–10)
- `get_market_sentiment(market_id)` — `GET /api/v1/news/sentiment/:marketId`; returns `MarketSentiment` with score (−100 to +100) and BULLISH / BEARISH / NEUTRAL label
- `provide_liquidity(params)` — `POST /api/v1/lp/provide`; accepts `&ProvideLiquidityParams`; returns `LpPosition` with buy and sell order IDs
- New types: `CalibrationBucket`, `AccuracyScore`, `PortfolioReview`, `MarketSentiment`, `ProvideLiquidityParams`, `LpPosition`

## [1.3.0] — 2026-03-30

### Added
- `get_arbitrage_opportunities(min_margin)` — `GET /api/v1/arbitrage`; returns `Vec<ArbitrageOpportunity>`
- `place_smart_order(params)` — `POST /api/v1/orders/smart`; accepts `&PlaceSmartOrderParams`
- `list_smart_orders()` — `GET /api/v1/orders/smart`; returns `Vec<SmartOrder>` with child progress
- `cancel_smart_order(id)` — `DELETE /api/v1/orders/smart/:id`
- `browse_marketplace(params)` — `GET /api/v1/marketplace`; accepts `&BrowseMarketplaceParams`
- `get_marketplace_listing(id)` — `GET /api/v1/marketplace/:id`; returns `MarketplaceListing`
- `purchase_strategy(listing_id)` — `POST /api/v1/marketplace/:id/purchase`; returns `MarketplacePurchaseResult`
- New types: `ArbitrageOpportunity`, `SmartOrderType`, `SmartOrderStatus`, `PlaceSmartOrderParams`, `PlaceSmartOrderResponse`, `SmartOrderChild`, `SmartOrder`, `MarketplaceListing`, `MarketplacePurchaseResult`, `BrowseMarketplaceParams`

## [1.2.0] — 2026-03-29

### Fixed
- `get_score()` path corrected: `/api/v1/score` → `/api/v1/scores/me`
- `get_whale_feed()` path corrected: `/api/v1/whale-feed` → `/api/v1/whales/feed`
- `get_news_signals()` path corrected: `/api/v1/news-signals` → `/api/v1/news/signals`
- `list_copy_configs()` path corrected: `/api/v1/copy-configs` → `/api/v1/copy`

### Added
- `update_strategy(id, name, description)` — `PATCH /api/v1/strategies/:id`
- `delete_strategy(id)` — `DELETE /api/v1/strategies/:id`
- `import_strategy(data)` — `POST /api/v1/strategies/import`
- `pause_strategy(id)` — `POST /api/v1/strategies/:id/pause`
- `resume_strategy(id)` — `POST /api/v1/strategies/:id/resume`
- `fork_strategy(id)` — `POST /api/v1/strategies/:id/fork`
- `close_position(params)` — `POST /api/v1/orders/close-position`
- `redeem_position(params)` — `POST /api/v1/orders/redeem`
- `split_position(params)` — `POST /api/v1/orders/split`
- `merge_positions(params)` — `POST /api/v1/orders/merge`
- `ListOrdersParams` now includes `strategy_id`, `from`, `to` filter fields
- `patch()` internal helper for `PATCH` requests
- New types: `ClosePositionParams`, `RedeemPositionParams`, `SplitPositionParams`, `MergePositionParams`

## [1.1.0] — 2026-03-29

### Added
- `watch_strategy(strategy_id)` — opens the strategy SSE stream and returns a `StrategyEventStream`
- `StrategyEventStream` struct — poll with `.next().await` to receive `Result<StrategyEvent>` items; drop to close the connection
- `StrategyEvent` struct — `event_type`, `strategy_id`, `data` (raw JSON value), `timestamp`
- Both `StrategyEventStream` and `StrategyEvent` re-exported from the crate root

## [1.0.0] — 2026-03-28

### Changed
- Bump version to 1.0.0 (aligned with TypeScript and Python SDKs)

### Fixed
- Align all API paths to canonical `/api/v1/*` pattern matching backend
- Add URL encoding for query parameters via `urlencoding` crate

### Added
- Unit tests for client construction, URL building, error types

## [0.2.0] — 2026-03-27

### Added
- `place_order()` — place direct buy/sell orders
- `cancel_order()` — cancel pending or live orders
- `PlaceOrderParams`, `PlaceOrderResponse`, `CancelOrderResponse` types

## [0.1.0] — 2026-03-27

### Added
- Initial release — async REST client for Polyforge API using reqwest
- 20 async methods covering all API endpoints
- Full Serde-derived type system
- `PolyforgeError` enum with Api, Http, Json variants
- rustls-tls by default, native-tls optional
