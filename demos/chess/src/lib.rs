//! Shared state and automation surfaces for the chess benchmark.
pub mod a11y;
pub mod app;
pub mod board;
pub mod games;
pub mod metrics;

pub use app::ChessApp;
pub use board::{Board, ClickOutcome};
pub use metrics::{Metrics, MetricsView};
