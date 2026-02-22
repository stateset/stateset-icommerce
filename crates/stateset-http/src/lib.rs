#![deny(unsafe_code)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! # StateSet HTTP
//!
//! HTTP service layer (REST + SSE) for the StateSet embedded commerce engine.
//!
//! Turns [`stateset_embedded::Commerce`] into a real HTTP API powered by
//! [`axum`], complete with JSON endpoints, pagination, Server-Sent Events,
//! CORS, request-ID tracing, and structured error responses.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use stateset_embedded::Commerce;
//! use stateset_http::ServerBuilder;
//! use std::net::SocketAddr;
//!
//! let commerce = Commerce::new(":memory:")?;
//! let addr: SocketAddr = "0.0.0.0:3000".parse()?;
//!
//! ServerBuilder::new(commerce)
//!     .bind(addr)
//!     .with_cors()
//!     .with_request_id()
//!     .serve()
//!     .await?;
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────┐
//! │                  HTTP Client                   │
//! │  ┌──────────────────────────────────────────┐  │
//! │  │         axum Router (this crate)         │  │
//! │  │  ┌────────────────────────────────────┐  │  │
//! │  │  │  stateset-embedded (Commerce)      │  │  │
//! │  │  │  ┌──────────────────────────────┐  │  │  │
//! │  │  │  │  SQLite / PostgreSQL         │  │  │  │
//! │  │  │  └──────────────────────────────┘  │  │  │
//! │  │  └────────────────────────────────────┘  │  │
//! │  └──────────────────────────────────────────┘  │
//! └────────────────────────────────────────────────┘
//! ```

mod dto;
mod error;
mod middleware;
pub mod routes;
mod server;
mod state;

pub use dto::*;
pub use error::HttpError;
pub use server::ServerBuilder;
pub use state::AppState;

// Re-export the router assembly function for users who want to embed the
// routes in a larger application.
pub use routes::api_router;
