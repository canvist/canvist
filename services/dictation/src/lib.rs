//! # `canvist_dictation_relay`
//!
//! The dictation relay: clients stream speech to it over the
//! [`canvist_dictation`] protocol, and it streams back text, as drafts that
//! rewrite themselves and finals that never change again.
//!
//! - **Providers:** OpenAI's Realtime transcription API, Deepgram's streaming
//!   API, or vLLM's realtime dialect for local models, each at a
//!   configurable base URL so servers that copy these APIs work too. API keys
//!   stay on the relay.
//! - **Voice activity detection:** an energy detector hears the end of each
//!   turn, for providers that need the relay to say where segments end.
//! - **Polish:** an optional language model (Claude, or any Chat
//!   Completions-compatible server) tidies each segment before it is final,
//!   and falls back to the raw transcript when it fails or is slow.
//! - **Order:** finals reach the client in segment order, however the
//!   provider and the polish pass interleave.
//!
//! ```no_run
//! use canvist_dictation_relay::Config;
//! use canvist_dictation_relay::Server;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let server = Server::start(Config::from_env()?).await?;
//! server.run_until_signal().await;
//! # Ok(())
//! # }
//! ```

mod app;
mod segments;
mod server;
mod session;

pub mod audio;
pub mod auth;
pub mod config;
pub mod polish;
pub mod provider;
pub mod vad;

pub use config::Config;
pub use server::Server;
pub use server::StartError;
