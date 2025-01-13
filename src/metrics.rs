use opentelemetry::{
    global,
    metrics::{Histogram, Meter},
    KeyValue,
};
use opentelemetry_semantic_conventions as semconv;
use std::{
    borrow::Cow,
    fmt::{self, Debug, Formatter},
    sync::Arc,
    time::Instant,
};
use trillium::{async_trait, Conn, Handler, KnownHeaderName, Status};

type StringExtractionFn = dyn Fn(&Conn) -> Option<Cow<'static, str>> + Send + Sync + 'static;
type StringAndPortExtractionFn =
    dyn Fn(&Conn) -> Option<(Cow<'static, str>, u16)> + Send + Sync + 'static;

/// Trillium handler that instruments http.server.request.duration, http.server.request.body.size,
/// and http.server.response.body.size as per [semantic conventions for http][http-metrics].
///
/// [http-metrics]: https://opentelemetry.io/docs/specs/semconv/http/http-metrics/
#[derive(Clone)]
pub struct Metrics {
    pub(crate) route: Option<Arc<StringExtractionFn>>,
    pub(crate) error_type: Option<Arc<StringExtractionFn>>,
    pub(crate) server_address_and_port: Option<Arc<StringAndPortExtractionFn>>,
    duration_histogram: Histogram<f64>,
    request_size_histogram: Histogram<u64>,
    response_size_histogram: Histogram<u64>,
}

impl Debug for Metrics {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Metrics")
            .field(
                "route",
                &match self.route {
                    Some(_) => "Some(..)",
                    _ => "None",
                },
            )
            .field(
                "error_type",
                &match self.error_type {
                    Some(_) => "Some(..)",
                    _ => "None",
                },
            )
            .field(
                "server_address_and_port",
                &match self.server_address_and_port {
                    Some(_) => "Some(..)",
                    _ => "None",
                },
            )
            .field("duration_histogram", &self.duration_histogram)
            .field("request_size_histogram", &self.request_size_histogram)
            .field("response_size_histogram", &self.response_size_histogram)
            .finish()
    }
}

/// Constructs a [`Metrics`] handler from a `&'static str`, [`Meter`], or [`&Meter`][Meter].
///
/// Alias for [`Metrics::new`] and [`Metrics::from`]
pub fn metrics(meter: impl Into<Metrics>) -> Metrics {
    meter.into()
}

impl From<&'static str> for Metrics {
    fn from(value: &'static str) -> Self {
        global::meter(value).into()
    }
}

impl From<Meter> for Metrics {
    fn from(value: Meter) -> Self {
        (&value).into()
    }
}

impl From<&Meter> for Metrics {
    fn from(meter: &Meter) -> Self {
        Self::builder(meter.clone()).build()
    }
}

impl Metrics {
    /// Constructs a new [`Metrics`] handler from a `&'static str`, [`&Meter`][Meter] or [`Meter`]
    pub fn new(meter: impl Into<Metrics>) -> Self {
        meter.into()
    }

    /// Creates a builder for [`Metrics`] from a `&'static str' or [`Meter`]
    pub fn builder(meter: impl Into<MetricsBuilder>) -> MetricsBuilder {
        meter.into()
    }

    /// provides a route specification to the metrics collector.
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
        self.route = Some(Arc::new(route));
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
        self.error_type = Some(Arc::new(error_type));
        self
    }

    /// Provides a callback for `server.address` and `server.port` attributes to the metrics
    /// collector.
    ///
    /// These should be set based on request headers according to the [OpenTelemetry HTTP semantic
    /// conventions][semconv-server-address-port].
    ///
    /// It is not recommended to enable this when the server is exposed to clients outside of your
    /// control, as request headers could arbitrarily increase the cardinality of these attributes.
    ///
    /// [semconv-server-address-port]:
    ///     https://opentelemetry.io/docs/specs/semconv/http/http-spans/#setting-serveraddress-and-serverport-attributes
    pub fn with_server_address_and_port<F>(mut self, server_address_and_port: F) -> Self
    where
        F: Fn(&Conn) -> Option<(Cow<'static, str>, u16)> + Send + Sync + 'static,
    {
        self.server_address_and_port = Some(Arc::new(server_address_and_port));
        self
    }
}

/// Configuration for [`Metrics`]
pub struct MetricsBuilder {
    meter: Meter,
    route: Option<Arc<StringExtractionFn>>,
    error_type: Option<Arc<StringExtractionFn>>,
    server_address_and_port: Option<Arc<StringAndPortExtractionFn>>,
}

impl Debug for MetricsBuilder {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("MetricsBuilder")
            .field("meter", &self.meter)
            .field(
                "route",
                &match self.route {
                    Some(_) => "Some(..)",
                    _ => "None",
                },
            )
            .field(
                "error_type",
                &match self.error_type {
                    Some(_) => "Some(..)",
                    _ => "None",
                },
            )
            .field(
                "server_address_and_port",
                &match self.server_address_and_port {
                    Some(_) => "Some(..)",
                    _ => "None",
                },
            )
            .finish()
    }
}

impl From<&'static str> for MetricsBuilder {
    fn from(name: &'static str) -> Self {
        global::meter(name).into()
    }
}

impl From<Meter> for MetricsBuilder {
    fn from(meter: Meter) -> Self {
        Self {
            meter,
            route: None,
            error_type: None,
            server_address_and_port: None,
        }
    }
}

impl MetricsBuilder {
    /// provides a route specification to the metrics collector.
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
        self.route = Some(Arc::new(route));
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
        self.error_type = Some(Arc::new(error_type));
        self
    }

    /// Provides a callback for `server.address` and `server.port` attributes to the metrics
    /// collector.
    ///
    /// These should be set based on request headers according to the [OpenTelemetry HTTP semantic
    /// conventions][semconv-server-address-port].
    ///
    /// It is not recommended to enable this when the server is exposed to clients outside of your
    /// control, as request headers could arbitrarily increase the cardinality of these attributes.
    ///
    /// [semconv-server-address-port]:
    ///     https://opentelemetry.io/docs/specs/semconv/http/http-spans/#setting-serveraddress-and-serverport-attributes
    pub fn with_server_address_and_port<F>(mut self, server_address_and_port: F) -> Self
    where
        F: Fn(&Conn) -> Option<(Cow<'static, str>, u16)> + Send + Sync + 'static,
    {
        self.server_address_and_port = Some(Arc::new(server_address_and_port));
        self
    }

    /// Constructs a new [`Metrics`] from the configuration.
    pub fn build(self) -> Metrics {
        let MetricsBuilder {
            meter,
            route,
            error_type,
            server_address_and_port,
        } = self;

        let duration_histogram = meter
            .f64_histogram(semconv::metric::HTTP_SERVER_REQUEST_DURATION)
            .with_description("Measures the duration of inbound HTTP requests.")
            .with_unit("s")
            .build();

        let request_size_histogram = meter
            .u64_histogram(semconv::metric::HTTP_SERVER_REQUEST_BODY_SIZE)
            .with_description("Measures the size of HTTP request messages (compressed).")
            .with_unit("By")
            .build();

        let response_size_histogram = meter
            .u64_histogram(semconv::metric::HTTP_SERVER_RESPONSE_BODY_SIZE)
            .with_description("Measures the size of HTTP response messages (compressed).")
            .with_unit("By")
            .build();

        Metrics {
            route,
            error_type,
            server_address_and_port,
            duration_histogram,
            request_size_histogram,
            response_size_histogram,
        }
    }
}

struct MetricsWasRun;

#[async_trait]
impl Handler for Metrics {
    async fn run(&self, conn: Conn) -> Conn {
        conn.with_state(MetricsWasRun)
    }

    async fn before_send(&self, mut conn: Conn) -> Conn {
        if conn.state::<MetricsWasRun>().is_none() {
            return conn;
        }

        let Metrics {
            route,
            error_type,
            server_address_and_port,
            duration_histogram,
            request_size_histogram,
            response_size_histogram,
        } = self.clone();
        let error_type = error_type.and_then(|et| et(&conn)).or_else(|| {
            let status = conn.status().unwrap_or(Status::NotFound);
            if status.is_server_error() {
                Some((status as u16).to_string().into())
            } else {
                None
            }
        });
        let status: i64 = (conn.status().unwrap_or(Status::NotFound) as u16).into();
        let route = route.and_then(|r| r(&conn));
        let start_time = conn.inner().start_time();
        let method = conn.method().as_str();
        let request_len = conn
            .request_headers()
            .get_str(KnownHeaderName::ContentLength)
            .and_then(|src| src.parse::<u64>().ok());
        let response_len = conn.response_len();
        let scheme = if conn.is_secure() { "https" } else { "http" };
        let version = conn
            .inner()
            .http_version()
            .as_str()
            .strip_prefix("HTTP/")
            .unwrap();
        let server_address_and_port = server_address_and_port.and_then(|f| f(&conn));

        let mut attributes = vec![
            KeyValue::new(semconv::attribute::HTTP_REQUEST_METHOD, method),
            KeyValue::new(semconv::attribute::HTTP_RESPONSE_STATUS_CODE, status),
            KeyValue::new(semconv::attribute::NETWORK_PROTOCOL_NAME, "http"),
            KeyValue::new(semconv::attribute::URL_SCHEME, scheme),
            KeyValue::new(semconv::attribute::NETWORK_PROTOCOL_VERSION, version),
        ];

        if let Some(error_type) = error_type {
            attributes.push(KeyValue::new("error.type", error_type));
        }

        if let Some(route) = route {
            attributes.push(KeyValue::new(semconv::attribute::HTTP_ROUTE, route))
        };

        if let Some((address, port)) = server_address_and_port {
            attributes.push(KeyValue::new(semconv::attribute::SERVER_ADDRESS, address));
            attributes.push(KeyValue::new(
                semconv::attribute::SERVER_PORT,
                i64::from(port),
            ));
        }

        conn.inner_mut().after_send(move |_| {
            let duration_s = (Instant::now() - start_time).as_secs_f64();

            duration_histogram.record(duration_s, &attributes);

            if let Some(response_len) = response_len {
                response_size_histogram.record(response_len, &attributes);
            }

            if let Some(request_len) = request_len {
                request_size_histogram.record(request_len, &attributes);
            }
        });

        conn
    }
}
