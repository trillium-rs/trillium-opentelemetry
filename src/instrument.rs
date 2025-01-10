use crate::{Metrics, Trace};
use opentelemetry::{
    global::{BoxedTracer, ObjectSafeTracer},
    InstrumentationScope,
};
use std::{borrow::Cow, sync::Arc};
use trillium::{Conn, HeaderName};
use trillium_macros::Handler;

/// a handler to send both traces and metrics in accordances with [semantic conventions for
/// http](https://opentelemetry.io/docs/specs/semconv/http/).
///
/// This is composed of a [`Trace`] handler and [`Metrics`] handler.
#[derive(Debug, Handler)]
pub struct Instrument((Trace<BoxedTracer>, Metrics));

/// construct an [`Instrument`] with the provided meter and tracer
pub fn instrument<T: ObjectSafeTracer + Send + Sync + 'static>(
    meter: impl Into<Metrics>,
    tracer: T,
) -> Instrument {
    Instrument::new(meter, tracer)
}

impl Instrument {
    /// construct a new [`Instrument`] with the provided meter and tracer
    pub fn new(
        meter: impl Into<Metrics>,
        tracer: impl ObjectSafeTracer + Send + Sync + 'static,
    ) -> Self {
        Self((Trace::new(BoxedTracer::new(Box::new(tracer))), meter.into()))
    }

    /// provides a route specification
    ///
    /// in order to avoid forcing anyone to use a particular router, this is provided as a
    /// configuration hook.
    ///
    /// for use with [`trillium-router`](https://docs.trillium.rs/trillium_router/index.html),
    /// ```
    /// use trillium_router::RouterConnExt;
    /// trillium_opentelemetry::Metrics::new(&opentelemetry::global::meter("example"))
    ///     .with_route(|conn| conn.route().map(|r| r.to_string().into()));
    /// ```
    pub fn with_route<F>(mut self, route: F) -> Self
    where
        F: Fn(&Conn) -> Option<Cow<'static, str>> + Send + Sync + 'static,
    {
        let route = Arc::new(route);
        self.0 .0.route = Some(route.clone());
        self.0 .1.route = Some(route);
        self
    }

    /// Provides an optional low-cardinality error type specification to the metrics collector.
    ///
    /// The implementation of this is application specific, but will often look like checking the
    /// [`Conn::state`] for an error enum and mapping that to a low-cardinality `&'static str`.
    pub fn with_error_type<F>(mut self, error_type: F) -> Self
    where
        F: Fn(&Conn) -> Option<Cow<'static, str>> + Send + Sync + 'static,
    {
        let error_type = Arc::new(error_type);
        self.0 .0.error_type = Some(error_type.clone());
        self.0 .1.error_type = Some(error_type);
        self
    }

    /// Provides a callback for `server.address` and `server.port` attributes to be used in metrics
    /// attributes. This has no effect on tracing span attributes, where `server.address` and
    /// `server.port` are always enabled.
    ///
    /// These should be set based on request headers according to the [OpenTelemetry HTTP semantic
    /// conventions][semconv-server-address-port].
    ///
    /// It is not recommended to enable this when the server is exposed to clients outside of your
    /// control, as request headers could arbitrarily increase the cardinality of these attributes.
    ///
    /// [semconv-server-address-port]:
    ///     https://opentelemetry.io/docs/specs/semconv/http/http-spans/#setting-serveraddress-and-serverport-attributes
    pub fn with_metrics_server_address_and_port<F>(mut self, server_address_and_port: F) -> Self
    where
        F: Fn(&Conn) -> Option<(Cow<'static, str>, u16)> + Send + Sync + 'static,
    {
        self.0 .1.server_address_and_port = Some(Arc::new(server_address_and_port));
        self
    }

    /// Specify a list of request headers to include in the trace spans
    pub fn with_headers(
        mut self,
        headers: impl IntoIterator<Item = impl Into<HeaderName<'static>>>,
    ) -> Self {
        self.0 .0.headers = headers.into_iter().map(Into::into).collect();
        self
    }

    /// Enable population of the local socket address and port in the trace spans.
    ///
    /// This populates the `network.local.address` and `network.local.port` attributes.
    pub fn with_local_address_and_port(mut self) -> Self {
        self.0 .0.enable_local_address_and_port = true;
        self
    }
}

/// The primary entrypoint if using [`opentelemetry::global`].
///
/// constructs a versioned meter and tracer with the name `"trillium-opentelemetry"`.
pub fn instrument_global() -> Instrument {
    instrument(
        opentelemetry::global::meter_provider().meter_with_scope(
            InstrumentationScope::builder("trillium-opentelemetry")
                .with_version(env!("CARGO_PKG_VERSION"))
                .with_schema_url("https://opentelemetry.io/schemas/1.29.0")
                .build(),
        ),
        opentelemetry::global::tracer("trillium-opentelemetry"),
    )
}
