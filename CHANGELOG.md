# Changelog

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
