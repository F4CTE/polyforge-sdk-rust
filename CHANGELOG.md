# Changelog

## [Unreleased]

### Added
- **Actions Catalog (POLA-3675)** — `get_actions()` method for `GET /api/v1/actions`. Returns the platform's public API actions catalog (`ActionsSchema`) for agent/tooling discovery, matching `getActions()` in sdk-ts and `get_actions()` in sdk-python. New types: `ActionsSchema`, `ActionDefinition`, `ActionParameter`. (closes #208)
- **Sports markets API** — 9 new `PolyforgeClient` methods wrapping the `/api/v1/sports/*` endpoints (POLA-1841):
  - `list_sports_categories()` → typed `Vec<SportsCategory>`
  - `list_sports_markets(params)` → `PaginatedResponse<serde_json::Value>`
  - `list_sports_events(params)` → `PaginatedResponse<serde_json::Value>`
  - `get_sports_event(event_ticker)` → `serde_json::Value` (`{ event, markets }`)
  - `list_sports_milestones(params)` → `serde_json::Value` (`{ milestones, cursor }`)
  - `get_sports_live_data(milestone_id)` → `serde_json::Value` (`{ liveData }`)
  - `list_sports_combos(params)` → `serde_json::Value` (`{ collections, cursor }`)
  - `get_sports_combo_collection(collection_ticker)` → `serde_json::Value`
  - `lookup_sports_combo(params)` → `serde_json::Value` (`{ eventTicker, marketTicker }` or `null`)
- New types: `SportsCategory`, `ListSportsMarketsParams`, `ListSportsEventsParams`,
  `ListSportsMilestonesParams`, `ListSportsCombosParams`, `SportsComboSelection`,
  `SportsComboLookupParams`. Weakly-typed payloads use `serde_json::Value` to
  mirror the controller's `Record<string, unknown>` / `unknown[]` fidelity instead
  of inventing strict shapes.
- **UpdateSettingsProfileParams.twitter_handle** — add optional `twitter_handle: Option<String>` field. Serializes as `twitterHandle` (camelCase) to match the platform's `UpdateProfileDto` and reach feature parity with `polyforge-sdk-python` and `polyforge-mcp`. (closes #185)
- **Misc public utility endpoints (POLA-1858)** — 18 read/write methods that close the SDK gap matrix from POLA-1845 and bring the Rust SDK to parity with the platform's miscellaneous user/markets/fees/analytics surface:
  - `get_accuracy_overview()` → `GET /api/v1/accuracy` — companion to `get_accuracy()` (`/accuracy/me`); both return the same `AccuracyScore` shape.
  - `get_feed(Option<&GetWhaleFeedParams>)` → `GET /api/v1/feed` — paged whale-trade feed (reuses existing `WhaleTrade` and `GetWhaleFeedParams` types since the controller delegates to `WhalesService.getFeed`).
  - `list_journal(Option<&ListJournalParams>)` → `GET /api/v1/journal` — order-journal entries with optional `mood` filter (`CONFIDENT | UNCERTAIN | FOMO | DISCIPLINED | REVENGE`).
  - `list_notifications(Option<&PaginationParams>)` → `GET /api/v1/notifications` — paginated notification records (distinct from existing `get_notification_settings()` / `get_notification_preferences()`, which return preference toggles).
  - `get_my_referrals()` → `GET /api/v1/referrals/me` — referral code, link, stats, and referrals list.
  - `preview_fees(&OrderPreviewParams)` → `POST /api/v1/fees/preview` — cross-venue fee comparison. Validates `size >= 1` and `0.001 <= price <= 0.999` client-side before the request hits the wire (mirrors `OrderPreviewDto` class-validator bounds).
  - `list_fee_schedules()` → `GET /api/v1/fees/schedules` — active venue fee schedules grouped by venue.
  - `list_market_alerts(market_id)` → `GET /api/v1/markets/{marketId}/alerts` — per-market alerts in the platform `{ data: [...] }` envelope (distinct from top-level `list_alerts()`).
  - `create_market_alert(market_id, &CreateMarketAlertParams)` → `POST /api/v1/markets/{marketId}/alerts`.
  - `delete_market_alert(market_id, alert_id)` → `DELETE /api/v1/markets/{marketId}/alerts/{alertId}` — server applies `ParseUUIDPipe` on `alert_id`.
  - `get_market_history(market_id, Option<MarketHistoryPeriod>)` → `GET /api/v1/markets/{marketId}/history` — `period` ∈ `1d | 7d | 30d | 90d`, defaults to `7d` server-side.
  - `get_market_sentiment_report(market_id)` → `GET /api/v1/markets/{marketId}/sentiment` — markets-controller sentiment report with `yesPercent`, `noPercent`, `totalVotes`, and nullable `userVote`. Method is named distinctly from the existing `get_market_sentiment(market_id)` (which still hits the news-controller `/news/sentiment/:marketId`) to avoid silently breaking callers.
  - `vote_market_sentiment(market_id)` → `POST /api/v1/markets/{marketId}/sentiment`.
  - `update_order_journal(order_id, &UpdateOrderJournalParams)` → `PATCH /api/v1/orders/{id}/journal`.
  - `list_combo_collections(Option<&ListComboCollectionsParams>)` → `GET /api/v1/markets/combo/collections` — Kalshi combo collections with optional `seriesTicker`/`limit`/`cursor` filters.
  - `get_combo_collection(ticker)` → `GET /api/v1/markets/combo/collections/{ticker}`.
  - `lookup_combo_market(&ComboLookupParams)` → `POST /api/v1/markets/combo/lookup` — `legs[].outcome` must be `"yes"` or `"no"` (lowercase) to match the server enum.
  - `get_correlation_categories()` → `GET /api/v1/analytics/correlation/categories` — top 20 market categories plus a square symmetric correlation matrix.

  New types: `JournalEntry`, `ListJournalParams`, `Notification`, `PaginationParams`, `ReferralStats`, `ReferralsInfo`, `PreviewSide`, `OrderPreviewParams`, `VenueFeeEstimate`, `MarketMatchRef`, `OrderPreviewResponse`, `PolymarketFeeSchedule`, `KalshiFeeSchedule`, `FeeSchedules`, `MarketAlertOutcome`, `MarketAlertCondition`, `MarketAlert`, `MarketAlertsResponse`, `CreateMarketAlertParams`, `MarketHistoryPeriod`, `MarketSentimentVote`, `MarketSentimentReport`, `OrderJournalMood`, `UpdateOrderJournalParams`, `ComboCollection`, `ListComboCollectionsParams`, `ComboLeg`, `ComboLookupParams`, `CategoryCorrelation`. Response types use `#[serde(flatten)] extra: serde_json::Value` for forward-compatibility with backend shape evolution.

  32 new unit tests cover URL paths, query/body camelCase serialization, validation bounds (size/price/non-finite inputs), enum casing, envelope handling, nullable sentiment votes, and JSON deserialization shapes. `cargo build`, `cargo clippy -- -D warnings`, and `cargo test` (321 unit tests plus 4 doc tests passing) are clean.

### Fixed
- **Trading writes** — automatically attach a fresh `Idempotency-Key` header to order, bulk order, liquidity, position, smart-order, and conditional-order mutations so platform idempotency validation no longer rejects Rust SDK writes with `MISSING_IDEMPOTENCY_KEY`. (closes #197)

### Notes
- Public user/profile lookup methods no longer send an `Authorization` header when
  the client was constructed with an empty API key. This keeps documented public
  endpoints usable without credentials while preserving authenticated behavior
  when a key is configured. Added multi-chunk coverage for the 1-MiB error body
  cap and documented that exactly 1 MiB is allowed while the first byte over the
  limit is rejected.
- Cross-SDK naming aliases: `get_notifications()` is now a deprecated alias for
  `list_notifications()`, and `ReferralsInfo` is now a deprecated alias for the
  canonical `MyReferralsResponse` type.
- `GET /sports/combos/:collectionTicker` currently ignores its path param
  server-side (forwards to `listComboCollections({page:1, limit:1})`). The SDK
  wraps the route as-is for fidelity; a server-side fix is tracked separately.
- **Public user profile lookups (POLA-1844)** — five endpoints sourced from the weekly SDK audit:
  - `get_user_performance(username, period)` → `Vec<UserPerformancePoint>` (PnL curve).
  - `get_user_strategies(username, visibility, limit)` → `Vec<UserStrategySummary>` (server caps `limit` at 50).
  - `get_user_activity(username, limit)` → `Vec<UserActivityEntry>` (resolved positions, server caps `limit` at 50).
  - `get_user_profile_badges(username)` → `Vec<UserProfileBadge>`.
  - `get_my_following(page, limit)` → `PaginatedResponse<FollowedUser>` (authenticated users only).

  All four public-profile endpoints surface `PolyforgeError::Api { status: 404, code: "NOT_FOUND", .. }` when the username is unknown. The `username` path segment is URL-encoded via the existing `urlencoding::encode` helper.

  New types: `UserPerformancePoint`, `UserStrategySummary`, `UserActivityEntry`, `UserProfileBadge`, `FollowedUser`, plus an internal `UserDataEnvelope<T>` to unwrap the `{ "data": [...] }` envelope.
- **Cross-venue arb execution / positions / risk endpoints (POLA-1852)** — 7 trading-impact-bearing methods that complete the `/api/v1/arbitrage/*` surface and bring the Rust SDK to parity with the Python SDK (POLA-1851):
  - `execute_arbitrage(&ExecuteArbitrageParams, idempotency_key)` → `POST /api/v1/arbitrage/execute`. Sends the backend-required `Idempotency-Key`, validates UUID `match_id`, integer `size` ∈ `1..=10000` USDC, and `max_slippage_pct` ∈ `0..=5` client-side before the order hits the wire (mirrors `ExecuteArbDto` `class-validator` bounds).
  - `list_arbitrage_positions(status, limit, offset)` → `GET /api/v1/arbitrage/positions`. Uses typed `ArbPositionStatus` and validates `limit` ∈ `1..=100`.
  - `get_arbitrage_position(id)` → `GET /api/v1/arbitrage/positions/{id}`.
  - `close_arbitrage_position(id, idempotency_key)` → `POST /api/v1/arbitrage/positions/{id}/close` with the backend-required `Idempotency-Key`.
  - `get_arbitrage_risk_dashboard()` → `GET /api/v1/arbitrage/risk/dashboard`.
  - `get_arbitrage_settlement_risks()` → `GET /api/v1/arbitrage/risk/settlement`.
  - `refresh_arbitrage_pnl()` → `POST /api/v1/arbitrage/risk/refresh-pnl`.
- New types: `ExecuteArbitrageParams`, `ArbPositionStatus`, `ArbExecutionLeg`, `ArbExecutionResult`, `ArbPosition`, `ArbPositionsResponse`, `ArbCloseResponse`, `ArbNetExposure`, `ArbRiskDashboard`, `ArbSettlementRisk`, `ArbPnlRefreshResult`. Decimal columns (`ArbExecutionLeg.price`, `buy_price`, `sell_price`, P&L, spread) are typed as `Option<String>` to preserve full precision from the backend Prisma `Decimal` serialization, matching the Python SDK shape.

### ⚠️ Trading impact (severity: HIGH)
- `execute_arbitrage()` and `close_arbitrage_position()` can place real offsetting orders. Callers must supply an idempotency key for safe retries; the SDK sends it as `Idempotency-Key` and fails fast on missing/invalid keys, malformed `match_id`, fractional or out-of-range `size`, invalid slippage, invalid position status filters, and oversized page limits before requests reach the trading API.

### Notes
- `GET /sports/combos/:collectionTicker` currently ignores its path param
  server-side (forwards to `listComboCollections({page:1, limit:1})`). The SDK
  wraps the route as-is for fidelity; a server-side fix is tracked separately.

### Fixed
- **Admin-only arbitrage match mutations** — hide `create_arbitrage_match`,
  `verify_arbitrage_match`, `delete_arbitrage_match`, and
  `sync_arbitrage_matches` from the public Rustdoc surface and deprecate them
  with an explicit admin-only note. The wrappers remain temporarily for source
  compatibility, but ordinary public API keys still receive `403 Forbidden`.
- **NotificationSettings / UpdateNotificationSettingsParams** — rewrite both structs to mirror the platform's `UpdateNotificationsDto`. Removes fictional fields (`pushEnabled`, `orderFills`, `strategyErrors`, `whaleAlerts`, `marketResolutions`, `dailySummary`) that the platform rejected with 400 under `forbidNonWhitelisted: true`, and adds the real DTO fields (`telegramEnabled`, `discordEnabled`, `onOrderFilled`, `onStrategyError`, `onBacktestComplete`, `onDailyLossLimit`, `onMarketResolved`, `onSomeoneForked`, `onSomeoneFollowed`, `onSomeoneLiked`, `onSomeoneCommented`). The `extra` flatten bucket is preserved on the read struct so server-only fields (`userId`, `updatedAt`, `eventPrefs`, `emailDigest`, `notificationFreq`, `minFillNotifyUsdc`, `onTicketReply`) round-trip. Added a wire-format key-set test. (closes #184)

## [1.7.6] — 2026-04-25

### Fixed
- **CreateAlertParams.price** — change type from `f64` to `String` to match platform's `@IsNumberString` validation. Alert creation was failing with a 400 error because the platform expects a numeric string, not a float. (closes #174)

## [1.7.5] — 2026-04-15

### Added
- **export_orders_csv()** — download order history as CSV text. (closes #121)
- **export_portfolio_csv()** — download portfolio positions as CSV text. (closes #121)
- Internal `get_text()` helper for endpoints that return non-JSON responses.

## [1.7.4] — 2026-04-15

### Fixed
- **GetPortfolioPnlParams** — replace `Option<String>` with typed `PnlPeriod` enum. Removes phantom `"1d"` value, renames `"all"` → `"allTime"`, adds missing `"90d"` variant. (closes #120)
- **ListOrdersParams** — add missing `page` and `market_id` query parameters to match platform `OrderQueryDto`. (closes #119)
- **ListConditionalOrdersParams** — add missing `order_type` (serialized as `type`) and `page` query parameters to match platform `ConditionalOrderQueryDto`. (closes #118)
- New `ConditionalOrderType` enum: `TakeProfit`, `StopLoss`, `TrailingStop`, `Limit`, `Pegged`

## [1.7.3] — 2026-04-15

### Fixed
- **CreateConditionalOrderParams** — add required `market_id` field, rename `condition_type` to `order_type` (serializes as `type`), add `trailing_pct` field. (closes #115)
- **Block struct** — replace `enabled: Option<bool>` with `connections: Vec<String>` to match platform's strategy block graph. (closes #116)
- **Backtest struct** — add 10 missing platform fields: `start_date`, `end_date`, `initial_balance`, `final_balance`, `pnl`, `trade_count`, `win_rate`, `sharpe_ratio`, `max_drawdown`, `completed_at`. (closes #117)

## [1.7.2] — 2026-04-15

### Fixed
- **Price history params** — `get_price_history()` now sends `period` (with values `"1h"`, `"6h"`, `"24h"`) instead of the incorrect `resolution` parameter. Removed unsupported `from`/`to` params that were silently ignored by the platform. (closes #114)

## [1.7.1] — 2026-04-14

### Security
- **Error response body size cap** — `handle_response()` now checks the `Content-Length` header on error responses and rejects bodies larger than 1 MiB (`MAX_RESPONSE_BODY_SIZE`), returning `PolyforgeError::Api` with code `RESPONSE_BODY_TOO_LARGE`; prevents memory amplification attacks where a malicious server returns an extremely large error body (closes #48)

## [1.7.0] — 2026-04-14

### Added
- **Backtest endpoints** — `list_backtests()` with query params (strategyId, status, page, limit), `get_backtest()`, `run_quick_backtest()`, `get_backtest_orders()`. New `ListBacktestsParams` struct. (closes #57, closes #74)

## [1.6.20] — 2026-04-14

### Fixed
- `browse_marketplace()`: `offset` query parameter was defined in `BrowseMarketplaceParams` but never serialized into the URL — users could not paginate past the first page of marketplace results (closes #28)

## [1.6.19] — 2026-04-14

### Fixed
- **BREAKING:** `Alert` struct: replaced phantom fields (`name`, `market_id`, `condition`, `enabled`) with correct platform fields (`token_id`, `direction`, `price`, `persistent`, `triggered`, `triggered_at`, `created_at`) matching `PriceAlert` Prisma model (closes #107)
- **BREAKING:** `Position` struct: added missing fields (`id`, `side`, `unrealized_pnl`, `realized_pnl`, `opened_at`) and removed incorrect `pnl` field — platform returns separate unrealized/realized PnL (closes #108)
- `StrategyTemplate` struct: added `blocks: Vec<Block>` and `popularity: u32` fields for template block configuration and usage score (closes #109)
- Added `#[serde(rename_all = "camelCase")]` to `StrategyTemplate` for consistent field deserialization

## [1.6.18] — 2026-04-13

### Added
- `list_conditional_orders(params)` — `GET /api/v1/orders/conditional` with optional status filter and limit (closes #40)
- `create_conditional_order(params)` — `POST /api/v1/orders/conditional` with financial parameter validation (closes #40)
- `get_conditional_order(order_id)` — `GET /api/v1/orders/conditional/:id` (closes #40)
- `cancel_conditional_order(order_id)` — `DELETE /api/v1/orders/conditional/:id` (closes #40)
- `create_alert(params)` — `POST /api/v1/alerts` with price validation (closes #40)
- `delete_alert(alert_id)` — `DELETE /api/v1/alerts/:id` (closes #40)
- `get_portfolio_pnl(params)` — `GET /api/v1/portfolio/pnl` with optional period and strategy filter (closes #40)
- New types: `ConditionalOrder`, `ConditionalOrderStatus`, `CreateConditionalOrderParams`, `ListConditionalOrdersParams`, `AlertDirection`, `CreateAlertParams`, `PortfolioPnl`, `GetPortfolioPnlParams`
- 18 new unit tests covering all new types and validation

## [1.6.17] — 2026-04-13

### Added
- `get_price_history(token_id, params)` — fetch price history candles for a market token with optional resolution, time range, and limit (closes #54)
- `get_order_book(token_id)` — fetch the current order book (bids/asks) for a market token (closes #54)
- `PriceHistoryParams` — typed query parameters for `get_price_history()`
- `PriceHistoryEntry` — single candle with timestamp, price, and optional volume
- `OrderBookLevel` — a price/size level in the order book
- `OrderBook` — order book snapshot with bids and asks
- 10 new unit tests covering all price history and order book types

## [1.6.16] — 2026-04-13

### Added
- `get_watchlist()` — list all markets on the user's watchlist (closes #55)
- `add_to_watchlist(market_id)` — add a market to the watchlist (closes #55)
- `remove_from_watchlist(market_id)` — remove a market from the watchlist (closes #55)
- `get_watchlist_status(market_id)` — check if a specific market is watched (closes #55)
- `delete_webhook(id)` — delete a registered webhook (closes #56)
- `test_webhook(id)` — send a test event to a webhook and return delivery result (closes #56)
- `WatchlistItem`, `WatchlistAddResponse`, `WatchlistStatus` types for watchlist responses
- `WebhookTestResult` type for webhook test responses
- 11 new unit tests covering all watchlist and webhook mutation types

## [1.6.15] — 2026-04-13

### Fixed
- `ListMarketsParams`: added `sort` and `closed` fields — users can now sort markets by volume/endDate/newest/etc. and filter by resolution status (closes #75)
- **BREAKING** `list_strategies()`: changed signature from `(Option<StrategyStatus>)` to `(&ListStrategiesParams)` — new `ListStrategiesParams` struct adds `sort`, `page`, and `limit` alongside the existing `status` filter, enabling full pagination and sorting (closes #77)

### Added
- `ListStrategiesParams` struct — typed query parameters for `list_strategies()`

## [1.6.14] — 2026-04-13

### Fixed
- **BREAKING** `WhaleTrade` struct: renamed `trader` to `wallet`, replaced `price` with `usd_value` (serde `"usdValue"`), added `market_name` — aligns with platform response shape (closes #23 partial)
- **BREAKING** `NewsSignal` struct: renamed `direction` to `sentiment`, changed `market_id: Option<String>` to `related_markets: Vec<String>` (serde `"relatedMarkets"`), renamed `timestamp` to `published_at` (serde `"publishedAt"`) — aligns with platform response shape (closes #23 partial)
- **BREAKING** `Alert` struct: added `name` and `last_triggered_at` (serde `"lastTriggeredAt"`), removed `threshold` — platform does not return threshold as a top-level field (closes #23 partial)
- **BREAKING** `AiQueryResponse` struct: changed `sources` from `Vec<serde_json::Value>` to `Vec<String>`, added `suggested_actions: Vec<String>` (serde `"suggestedActions"`) — aligns with platform response shape (closes #23 partial)
- **BREAKING** `SplitPositionParams`: replaced `size: f64` and `price: f64` with `amount: String` — platform `SplitPositionDto` expects `{tokenId, amount}` not `{tokenId, size, price}` (closes #30 partial)
- **BREAKING** `MergePositionParams`: replaced `token_ids: Vec<String>` with `token_id: String` and `amount: String` — platform `MergePositionDto` expects `{tokenId, amount}` not `{tokenIds: [...]}`, renamed `merge_positions()` to `merge_position()` (closes #30 partial)
- **BREAKING** `ProvideLiquidityParams`: replaced `token_id` with `market_id` (serde `"marketId"`), removed `spread` — platform `ProvideLiquidityDto` expects `{marketId, size}` not `{tokenId, spread, size}` (closes #30 partial)
- **BREAKING** `RedeemPositionParams`: renamed `token_id` to `position_id` (serde `"positionId"`), renamed `condition_id` to `market_id` (serde `"marketId"`) — platform `RedeemPositionDto` expects `{positionId, marketId}` not `{tokenId, conditionId}` (closes #31)
- **BREAKING** `import_strategy()`: changed signature from `(&Value)` to `(polyforge_version, &Value, Option<&str>)` — platform `ImportStrategyDto` requires `{polyforge, strategy}` not `{data}` (closes #32)
- Issues #19 and #20 were already fixed in v1.6.11 — whale feed sends `minSize` and news signals sends `minConfidence` correctly (closes #19, closes #20)
- Issue #22 (Market name vs title) was already fixed in v1.6.11 — Market struct uses `name` field correctly (closes #22)

### Added
- 14 new unit tests covering all struct/payload fixes

## [1.6.13] — 2026-04-13

### Fixed
- **BREAKING** `ClosePositionParams.size`: changed from `Option<f64>` to `Option<String>` — platform's `ClosePositionDto.size` uses `@IsNumberString()` which requires a string value, causing 400 Bad Request on every `close_position()` call with a size (closes #33)
- **BREAKING** `Order.status`: changed from `Option<String>` to `Option<OrderStatus>` — added typed `OrderStatus` enum with all 12 platform variants (PENDING, SUBMITTED, LIVE, MATCHED, DELAYED, MINED, CONFIRMED, PARTIAL, CANCELLED, UNMATCHED, FAILED, ERROR); `ListOrdersParams.status` also changed to `Option<OrderStatus>` for compile-time safety (closes #34)
- **BREAKING** `StrategyStatus`: added missing `Error` and `Archived` variants — deserialization failed when platform returned `"ERROR"` or `"ARCHIVED"` status, breaking `list_strategies()` and `get_strategy()` (closes #35)
- **BREAKING** `Strategy` struct: added typed `triggers`, `conditions`, `actions`, `safety` (`Vec<Block>`), `logic_blocks`, `calc_blocks`, `visibility` (`Visibility` enum), `exec_mode` (`ExecMode` enum), `tick_ms`, `tags`, `version`, `fork_count`, `like_count` fields — strategy blocks were only accessible through the untyped `extra` catch-all (closes #36)
- **BREAKING** `create_strategy()`: changed signature from `(name, description, market_id)` to `(&CreateStrategyParams)` — new `CreateStrategyParams` struct includes all 15 platform DTO fields (blocks, visibility, execMode, tickMs, tags, variables, canvas) so strategies can actually be created with block configuration (closes #37)
- **BREAKING** `CopyConfig` struct: renamed `source_trader` to `target_wallet` (serde `"targetWallet"`), renamed `max_size` to `max_exposure` (serde `"maxExposure"`, type changed to `Option<String>`), added `mode` (`CopyMode` enum: PERCENTAGE/FIXED/MIRROR), `size_value`, `max_daily_loss`, `price_offset` fields — all fields were either wrong-named or missing, causing silent data loss on deserialization (closes #51)
- Added `run_backtest()` method with `RunBacktestParams` struct matching platform's `CreateBacktestDto` (strategyId, dateRangeStart, dateRangeEnd, quickMode, strategyBlocks, marketBindings) — does NOT include `initialBalance` which the platform silently discards (closes #68)

### Added
- `OrderStatus` enum — 12-variant typed enum for order lifecycle status
- `Visibility` enum — PRIVATE, PUBLIC, UNLISTED
- `ExecMode` enum — TICK, EVENT, HYBRID
- `CopyMode` enum — PERCENTAGE, FIXED, MIRROR
- `Block`, `LogicBlock`, `CalcBlock` structs — typed strategy block representations
- `CreateStrategyParams` struct — full strategy creation with all platform DTO fields
- `RunBacktestParams` struct — backtest parameters matching platform contract
- `Backtest` struct — backtest result type
- 15 new unit tests covering all 7 fixes

## [1.6.12] — 2026-04-13

### Security
- **SSRF protection for base URL** — `validate_base_url()` now blocks private IPs (10.x, 172.16-31.x, 192.168.x), link-local (169.254.x), CGNAT (100.64.0.0/10), cloud metadata hostnames (`.internal`, `.local`), and IPv6 reserved ranges; reuses the existing `is_blocked_ip()` helper shared with `validate_webhook_url()`; localhost/127.0.0.1/[::1] remain exempted for development use — prevents API keys from being sent to attacker-controlled internal addresses via `PolyforgeClient::with_url()` (closes #26)

## [1.6.11] — 2026-04-13

### Fixed
- **BREAKING** `Order` and `Position` monetary fields (`size`, `price`, `fill_size`, `fill_price`, `pnl`, `avg_price`, `current_price`): changed from `Option<f64>` to `Option<String>` — platform returns decimal strings (`"0.65"`), not JSON numbers, causing serde deserialization failures (closes #38)
- **BREAKING** `get_whale_feed()`: send `?minSize=X` instead of `?min_size=X`; `get_news_signals()`: send `?minConfidence=X` instead of `?min_confidence=X` — platform expects camelCase query parameters (closes #39)
- **BREAKING** `Market` struct: rename `title` to `name`, replace `tokens: Vec<Token>` with `base_token`/`quote_token` fields, rename `volume` to `volume_24h` (serde `"volume24h"`), add `price`, `change_24h`, `liquidity`, `created_at` — aligns with actual platform response shape (closes #58)
- **BREAKING** `Portfolio` struct: rename `balance` to `available_balance` (serde `"availableBalance"`), replace `total_pnl` with `unrealized_pnl`/`realized_pnl`, change monetary fields to `Option<String>`, add `updated_at` — aligns with actual platform response shape (closes #59)
- **BREAKING** `TraderScore` struct: rename `score` to `overall`, remove phantom fields (`total_trades`, `win_rate`, `profit_factor`), add `profitability`, `consistency`, `risk_management`, `volume`, `percentile`, `updated_at` — aligns with actual platform response shape (closes #60)

## [1.6.10] — 2026-04-13

### Fixed
- Add optional `suggestion` field to `PolyforgeError::Api` variant — the platform returns an optional `suggestion` string in error JSON bodies but it was being silently dropped; now extracted and available to callers (closes #93)

## [1.6.9] — 2026-04-13

### Fixed
- **BREAKING** Error response parsing: read `requestId` (camelCase) instead of `request_id` (snake_case) — `PolyforgeError::Api.request_id` was always `None` even when the platform included a request ID (closes #49)
- **BREAKING** Strategy lifecycle methods (`start_strategy`, `stop_strategy`, `pause_strategy`, `resume_strategy`): return `StrategyStatusResponse` instead of `Strategy` — the platform returns `{"status":"RUNNING"}` not a full strategy object, causing serde deserialization failure on every lifecycle call (closes #65)
- **BREAKING** List endpoints (`list_strategies`, `get_orders`, `get_whale_feed`, `get_news_signals`, `list_alerts`, `list_copy_configs`, `list_webhooks`, `get_strategy_templates`, `list_smart_orders`): return `PaginatedResponse<T>` instead of `Vec<T>` — the platform wraps all list responses in `{"data":[...],"total":N,...}`, causing `expected a sequence, found a map` deserialization failure (closes #76)

## [1.6.8] — 2026-04-13

### Fixed
- **BREAKING** `ai_query()`: send `{ "query" }` instead of `{ "question" }` to match platform `AiQueryDto` — AI queries were returning HTTP 400 (closes #89, regression of #50)
- **BREAKING** `create_strategy_from_description()`: send `{ "description" }` instead of `{ "query" }` to match platform `CreateFromDescriptionDto` — AI strategy creation was returning HTTP 400 (closes #90, regression of #44)
- **BREAKING** `WebhookEvent`: change serde rename values from dot.notation (`order.filled`) to SCREAMING_SNAKE_CASE (`ORDER_FILLED`) to match platform `CreateWebhookDto` validation — webhook creation was returning HTTP 400 (closes #91, regression of #47)
- **BREAKING** `TradingMode`: change serde rename from uppercase (`LIVE`/`PAPER`) to lowercase (`live`/`paper`) to match platform `StartStrategyDto` — strategy start was returning HTTP 400 (closes #92, regression of #46)

## [Unreleased]

### Security
- **Client-side financial parameter validation** — `place_order()`, `place_smart_order()`, and `provide_liquidity()` now reject NaN, Infinity, zero, and negative values for all financial parameters (size, price, total_size, spread, and optional price fields) before sending requests; prevents nonsensical orders from reaching the backend (closes #88)


- **Default URL updated to production** — changed `DEFAULT_BASE_URL` from `https://localhost:3002` to `https://api.polyforge.app` to match Python and TypeScript SDKs; localhost with HTTPS causes immediate TLS failures for new users and encourages insecure workarounds (closes #87, regression of #71)
- **SSE buffer overflow protection** — `StrategyEventStream` now enforces a 1 MiB (`MAX_SSE_BUFFER_SIZE`) cap on its internal line buffer; if the server sends data without a newline beyond this limit the stream returns `PolyforgeError::Api` with code `SSE_BUFFER_OVERFLOW` instead of growing unboundedly toward OOM (closes #52)
- **Cargo.lock committed for reproducible builds** — removed `Cargo.lock` from `.gitignore` and committed the lockfile so that `cargo audit` can run, builds are reproducible, and supply-chain attacks are detectable via dependency pinning (closes #42)
- **DNS rebinding SSRF mitigation** — `validate_webhook_url()` now resolves domain names via `tokio::net::lookup_host()` and checks all resolved IPs against the private/loopback blocklist, preventing SSRF bypass via attacker-controlled DNS records; also adds CGNAT range (100.64.0.0/10, RFC 6598) to the blocklist; documents that this is a client-side best-effort check and the server must independently validate (closes #63, closes #41)
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
