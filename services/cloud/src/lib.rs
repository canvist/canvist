//! # `canvist_cloud`
//!
//! The server behind canvist Cloud, and the exact binary self-hosters run.
//!
//! - **Sync** (`/v1/sync`, `/v1/hocuspocus`): a multi-tenant y-sync relay
//!   that stock y-websocket and Hocuspocus clients connect to. Documents
//!   persist to Postgres as updates plus compacted snapshots, idle ones are
//!   evicted from memory, and changes fire debounced, signed webhooks.
//! - **API** (`/v1/tokens`, `/v1/documents`, `/v1/webhook`): API-key
//!   authenticated management for a customer's servers.
//! - **Admin** (`/v1/admin`): the dashboard at canvist.dev/admin, signed in
//!   by email: projects, Stripe coupons, and analytics.
//! - **Analytics** (`/v1/events`): first-party page views and clicks from
//!   the website.
//!
//! The same binary serves every route group; `CANVIST_ROLE` picks which, so
//! Railway can scale each product as its own service from one image.
//!
//! ```no_run
//! use canvist_cloud::Config;
//! use canvist_cloud::Server;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let server = Server::start(Config::from_env()?).await?;
//! server.run_until_signal().await;
//! # Ok(())
//! # }
//! ```

mod admin;
mod api;
mod app;
mod convert;
mod documents;
mod error;
mod server;

pub mod analytics;
pub mod auth;
pub mod config;
pub mod metering;
pub mod names;
pub mod store;
pub mod sync;

pub use app::App;
pub use config::Config;
pub use server::Server;
pub use server::ShutdownReport;
pub use server::StartError;
