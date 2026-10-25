//! Request and response types of the review-tools JSON API, the review document format
//! (parser and differ), decision ids and versions. No I/O.

/// Version of the JSON API this crate describes; part of every API path (`/api/v1/...`).
pub const API_VERSION: u32 = 1;

pub mod diff;
pub mod doc;
pub mod model;
pub mod remote;
pub mod secrets;

pub use model::*;
