//! # Polyforge SDK
//!
//! Async Rust client for the Polyforge trading platform REST API.
//!
//! ## Quick Start
//!
//! ```no_run
//! use polyforge::PolyforgeClient;
//!
//! #[tokio::main]
//! async fn main() -> polyforge::Result<()> {
//!     let client = PolyforgeClient::new("your-api-key")?;
//!     let markets = client.list_markets(&Default::default()).await?;
//!     println!("Found {} markets", markets.total);
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod errors;
pub mod types;
pub mod ws;

pub use client::{PolyforgeClient, StrategyEventStream};
pub use errors::{PolyforgeError, Result};
pub use types::*;

pub use ws::*;

pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");
