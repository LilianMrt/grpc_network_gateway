//! Library surface for the gateway.
//!
//! `main.rs` is a thin binary over this crate so that examples and the
//! `tests/` directory can link against the same modules and generated
//! protobuf types. Integration tests cannot reach into a binary target.

pub mod network;
pub mod services;
