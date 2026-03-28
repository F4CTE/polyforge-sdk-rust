# Changelog

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
