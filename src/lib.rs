//! This crate provides opentelemetry metrics conforming to
//! [semantic conventions for http](https://opentelemetry.io/docs/reference/specification/metrics/semantic_conventions/http-metrics/).
#![forbid(unsafe_code)]
#![deny(
    missing_copy_implementations,
    rustdoc::missing_crate_level_docs,
    missing_debug_implementations,
    nonstandard_style,
    unused_qualifications,
    missing_docs
)]
pub use opentelemetry;

#[cfg(all(feature = "trace", feature = "metrics"))]
mod instrument;
#[cfg(feature = "metrics")]
mod metrics;
#[cfg(feature = "trace")]
mod trace;

#[cfg(feature = "trace")]
mod instrument_handler;

#[cfg(all(feature = "trace", feature = "metrics"))]
pub use instrument::{instrument, Instrument};
#[cfg(feature = "trace")]
pub use instrument_handler::{instrument_handler, InstrumentHandler};
#[cfg(feature = "metrics")]
pub use metrics::{metrics, Metrics};
#[cfg(feature = "trace")]
pub use trace::{trace, Trace};

#[cfg(all(feature = "trace", feature = "metrics"))]
/// instrumentation using [`opentelemetry::global`]
pub mod global {
    pub use super::instrument::instrument_global as instrument;
    pub use super::instrument_handler::instrument_handler_global as instrument_handler;
}
