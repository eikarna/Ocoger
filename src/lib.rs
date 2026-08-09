//! Library facade exposing the domain modules for binary + integration tests.
//! Binary entry (`main.rs`) wires logging + event loop; the heavy lifters remain
//! in `core`, `services`, and `ui`.

pub mod core;
pub mod services;
pub mod ui;
