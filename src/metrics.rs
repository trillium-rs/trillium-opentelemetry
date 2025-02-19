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
use trillium::{async_trait, log, Conn, Handler, Info, KnownHeaderName, Status};

type StringExtractionFn = dyn Fn(&Conn) -> Option<Cow<'static, str>> + Send + Sync + 'static;
type StringAndPortExtractionFn =
    dyn Fn(&Conn) -> Option<(Cow<'static, str>, u16)> + Send + Sync + 'static;

/// Trillium handler that instruments http.server.request.duration, http.server.request.body.size,
/// and http.server.response.body.size as per [semantic conventions for http][http-metrics].
///
/// [http-metrics]: https://opentelemetry.io/docs/specs/semconv/http/http-metrics/
pub struct Metrics {
    pub(crate) route: Option<Arc<StringExtractionFn>>,
    pub(crate) error_type: Option<Arc<StringExtractionFn>>,
    pub(crate) server_address_and_port: Option<Arc<StringAndPortExtractionFn>>,
    pub(crate) histograms: Histograms,
}

#[derive(Clone, Debug)]
pub(crate) enum Histograms {
    Uninitialized {
        meter: Meter,
        duration_histogram_boundaries: Option<Vec<f64>>,
        request_size_histogram_boundaries: Option<Vec<f64>>,
        response_size_histogram_boundaries: Option<Vec<f64>>,
    },
    Initialized {
        duration_histogram: Histogram<f64>,
        request_size_histogram: Histogram<u64>,
        response_size_histogram: Histogram<u64>,
    },
}

impl Histograms {
    fn init(&mut self) {
        match self {
            Self::Uninitialized {
                meter,
                duration_histogram_boundaries,
                request_size_histogram_boundaries,
                response_size_histogram_boundaries,
            } => {
                let mut duration_histogram_builder = meter
                    .f64_histogram(semconv::metric::HTTP_SERVER_REQUEST_DURATION)
                    .with_description("Measures the duration of inbound HTTP requests.")
                    .with_unit("s");
                duration_histogram_builder.boundaries = duration_histogram_boundaries.take();

                let mut request_size_histogram_builder = meter
                    .u64_histogram(semconv::metric::HTTP_SERVER_REQUEST_BODY_SIZE)
                    .with_description("Measures the size of HTTP request messages (compressed).")
                    .with_unit("By");
                request_size_histogram_builder.boundaries =
                    request_size_histogram_boundaries.take();

                let mut response_size_histogram_builder = meter
                    .u64_histogram(semconv::metric::HTTP_SERVER_RESPONSE_BODY_SIZE)
                    .with_description("Measures the size of HTTP response messages (compressed).")
                    .with_unit("By");
                response_size_histogram_builder.boundaries =
                    response_size_histogram_boundaries.take();

                *self = Self::Initialized {
                    duration_histogram: duration_histogram_builder.build(),
                    request_size_histogram: request_size_histogram_builder.build(),
                    response_size_histogram: response_size_histogram_builder.build(),
                }
            }

            Self::Initialized { .. } => {
                log::warn!("Attempted to initialize the Metrics handler twice");
            }
        }
    }

    fn set_request_size_boundaries(&mut self, boundaries: Vec<f64>) {
        match self {
            Self::Uninitialized {
                request_size_histogram_boundaries,
                ..
            } => {
                *request_size_histogram_boundaries = Some(boundaries);
            }

            Self::Initialized { .. } => {
                log::warn!("Attempted to set histogram boundaries on a Metrics handler that was already initialized");
            }
        }
    }

    fn set_response_size_boundaries(&mut self, boundaries: Vec<f64>) {
        match self {
            Self::Uninitialized {
                response_size_histogram_boundaries,
                ..
            } => {
                *response_size_histogram_boundaries = Some(boundaries);
            }

            Self::Initialized { .. } => {
                log::warn!("Attempted to set histogram boundaries on a Metrics handler that was already initialized");
            }
        }
    }

    fn set_duration_boundaries(&mut self, boundaries: Vec<f64>) {
        match self {
            Self::Uninitialized {
                duration_histogram_boundaries,
                ..
            } => {
                *duration_histogram_boundaries = Some(boundaries);
            }
            Self::Initialized { .. } => {
                log::warn!("Attempted to set histogram boundaries on a Metrics handler that was already initialized");
            }
        }
    }

    fn record_duration(&self, duration_s: f64, attributes: &[KeyValue]) {
        match self {
            Self::Initialized {
                duration_histogram, ..
            } => {
                duration_histogram.record(duration_s, attributes);
            }
            Self::Uninitialized { .. } => {
                log::error!("Attempted to record a duration on an uninitialized Metrics handler");
            }
        }
    }
    fn record_response_len(&self, response_len: u64, attributes: &[KeyValue]) {
        match self {
            Self::Initialized {
                response_size_histogram,
                ..
            } => {
                response_size_histogram.record(response_len, attributes);
            }

            Self::Uninitialized { .. } => {
                log::error!(
                    "Attempted to record a response length on an uninitialized Metrics handler"
                );
            }
        }
    }

    fn record_request_len(&self, request_len: u64, attributes: &[KeyValue]) {
        match self {
            Self::Initialized {
                request_size_histogram,
                ..
            } => {
                request_size_histogram.record(request_len, attributes);
            }

            Self::Uninitialized { .. } => {
                log::error!(
                    "Attempted to record a request length on an uninitialized Metrics handler"
                );
            }
        }
    }
}

impl From<Histograms> for Metrics {
    fn from(value: Histograms) -> Self {
        Metrics {
            route: None,
            error_type: None,
            server_address_and_port: None,
            histograms: value,
        }
    }
}

impl From<Meter> for Histograms {
    fn from(meter: Meter) -> Self {
        Histograms::Uninitialized {
            meter,
            duration_histogram_boundaries: None,
            request_size_histogram_boundaries: None,
            response_size_histogram_boundaries: None,
        }
    }
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
            .field("histograms", &self.histograms)
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
        Histograms::from(value).into()
    }
}

impl From<&Meter> for Metrics {
    fn from(meter: &Meter) -> Self {
        meter.clone().into()
    }
}

impl Metrics {
    /// Constructs a new [`Metrics`] handler from a `&'static str`, [`&Meter`][Meter] or [`Meter`]
    pub fn new(meter: impl Into<Metrics>) -> Self {
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

    /// Sets histogram boundaries for request durations (in seconds).
    ///
    /// This sets the histogram bucket boundaries for the [`http.server.request.duration`][semconv]
    /// metric.
    ///
    /// [semconv]: https://opentelemetry.io/docs/specs/semconv/http/http-metrics/#metric-httpserverrequestduration
    pub fn with_duration_histogram_boundaries(mut self, boundaries: Vec<f64>) -> Self {
        self.histograms.set_duration_boundaries(boundaries);
        self
    }

    /// Sets histogram boundaries for request sizes (in bytes).
    ///
    /// This sets the histogram bucket boundaries for the [`http.server.request.body.size`][semconv]
    /// metric.
    ///
    /// [semconv]: https://opentelemetry.io/docs/specs/semconv/http/http-metrics/#metric-httpserverrequestbodysize
    pub fn with_request_size_histogram_boundaries(mut self, boundaries: Vec<f64>) -> Self {
        self.histograms.set_request_size_boundaries(boundaries);
        self
    }

    /// Sets histogram boundaries for response sizes (in bytes).
    ///
    /// This sets the histogram bucket boundaries for the [`http.server.response.body.size`][semconv]
    /// metric.
    ///
    /// [semconv]: https://opentelemetry.io/docs/specs/semconv/http/http-metrics/#metric-httpserverresponsebodysize
    pub fn with_response_size_histogram_boundaries(mut self, boundaries: Vec<f64>) -> Self {
        self.histograms.set_response_size_boundaries(boundaries);
        self
    }
}

struct MetricsWasRun;

#[async_trait]
impl Handler for Metrics {
    async fn run(&self, conn: Conn) -> Conn {
        conn.with_state(MetricsWasRun)
    }

    async fn init(&mut self, _: &mut Info) {
        self.histograms.init();
    }

    async fn before_send(&self, mut conn: Conn) -> Conn {
        if conn.state::<MetricsWasRun>().is_none() {
            return conn;
        }

        let Metrics {
            route,
            error_type,
            server_address_and_port,
            histograms,
        } = self;
        let error_type = error_type.as_ref().and_then(|et| et(&conn)).or_else(|| {
            let status = conn.status().unwrap_or(Status::NotFound);
            if status.is_server_error() {
                Some((status as u16).to_string().into())
            } else {
                None
            }
        });
        let status: i64 = (conn.status().unwrap_or(Status::NotFound) as u16).into();
        let route = route.as_ref().and_then(|r| r(&conn));
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
        let server_address_and_port = server_address_and_port.as_ref().and_then(|f| f(&conn));

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

        let histograms = histograms.clone();
        conn.inner_mut().after_send(move |_| {
            let duration_s = (Instant::now() - start_time).as_secs_f64();

            histograms.record_duration(duration_s, &attributes);

            if let Some(response_len) = response_len {
                histograms.record_response_len(response_len, &attributes);
            }

            if let Some(request_len) = request_len {
                histograms.record_request_len(request_len, &attributes);
            }
        });

        conn
    }
}
