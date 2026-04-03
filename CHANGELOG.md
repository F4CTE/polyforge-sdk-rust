# Changelog

## [1.4.3] — 2026-04-03

### Fixed
- `create_strategy_from_description`: send `"marketId"` instead of `"market_id"` in request body to match platform API (closes #14)

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
