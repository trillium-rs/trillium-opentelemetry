use opentelemetry::{
    trace::{SpanBuilder, SpanKind, TraceContextExt, Tracer},
    Array, Context, KeyValue, Value,
};
use std::{
    borrow::Cow,
    fmt::{self, Debug, Formatter},
    net::SocketAddr,
    sync::Arc,
    time::{Instant, SystemTime},
};
use trillium::{async_trait, Conn, Handler, HeaderName, KnownHeaderName, Status};

type StringExtractionFn = dyn Fn(&Conn) -> Option<Cow<'static, str>> + Send + Sync + 'static;

/// Trillium handler that instruments per-request spans as per [semantic conventions for http][http-spans].
///
/// [http-spans]: https://opentelemetry.io/docs/specs/semconv/http/http-spans
#[derive(Clone)]
pub struct Trace<T> {
    pub(crate) route: Option<Arc<StringExtractionFn>>,
    pub(crate) error_type: Option<Arc<StringExtractionFn>>,
    pub(crate) headers: Vec<HeaderName<'static>>,
    tracer: T,
    socket_addr: Option<SocketAddr>,
}

impl<Span> Debug for Trace<Span> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Trace")
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
            .field("tracer", &"..")
            .finish()
    }
}

/// Alias for [`Trace::new`]
pub fn trace<T: Tracer>(tracer: T) -> Trace<T> {
    Trace::new(tracer)
}

impl<T: Tracer> Trace<T> {
    /// Constructs a new [`Trace`] handler from a Tracer
    pub fn new(tracer: T) -> Self {
        Trace {
            route: None,
            error_type: None,
            tracer,
            headers: vec![],
            socket_addr: None,
        }
    }

    /// provides a route specification to include in the trace spans.
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

    /// Provides an optional low-cardinality error type specification to include in the trace spans.
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

    /// Specify a list of request headers to include in the trace spans
    pub fn with_headers(
        mut self,
        headers: impl IntoIterator<Item = impl Into<HeaderName<'static>>>,
    ) -> Self {
        self.headers = headers.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TraceContext {
    pub(crate) context: Context,
}

struct RouteWasAvailable;

#[async_trait]
impl<T> Handler for Trace<T>
where
    T: Tracer + Send + Sync + 'static,
    T::Span: Send + Sync + 'static,
{
    async fn init(&mut self, info: &mut trillium::Info) {
        self.socket_addr = info.tcp_socket_addr().cloned();
    }
    async fn run(&self, mut conn: Conn) -> Conn {
        let start_time =
            Some(SystemTime::now() - conn.inner().start_time().duration_since(Instant::now()));

        let scheme = if conn.is_secure() { "https" } else { "http" };
        let method = conn.method().as_str();

        let version = conn
            .inner()
            .http_version()
            .as_str()
            .strip_prefix("HTTP/")
            .unwrap();

        let mut attributes = vec![
            KeyValue::new("http.request.method", method),
            KeyValue::new("url.path", conn.inner().path().to_string()),
            KeyValue::new("url.scheme", scheme),
            KeyValue::new("url.query", conn.inner().querystring().to_string()),
            KeyValue::new("network.protocol.name", "http"),
            KeyValue::new("network.protocol.version", version),
        ];

        if let Some(socket_addr) = &self.socket_addr {
            attributes.push(KeyValue::new(
                "network.local.address",
                socket_addr.ip().to_string(),
            ));

            attributes.push(KeyValue::new(
                "network.local.port",
                i64::from(socket_addr.port()),
            ));
        }

        if let Some(peer_ip) = conn.inner().peer_ip() {
            attributes.push(KeyValue::new("client.address", peer_ip.to_string()));
        }

        for (header_name, header_values) in self.headers.iter().filter_map(|hn| {
            conn.request_headers()
                .get_values(hn.clone())
                .map(|v| (hn, v))
        }) {
            attributes.push(KeyValue::new(
                format!(
                    "http.request.header.{}",
                    header_name.as_ref().to_lowercase()
                ),
                Value::Array(Array::String(
                    header_values.iter().map(|x| x.to_string().into()).collect(),
                )),
            ));
        }

        let address_and_port = conn.inner().host().map(|host| {
            host.split_once(':')
                .and_then(|(host, port)| Some((String::from(host), port.parse().ok()?)))
                .unwrap_or_else(|| (String::from(host), if conn.is_secure() { 443 } else { 80 }))
        });

        if let Some((address, port)) = address_and_port {
            attributes.push(KeyValue::new("server.address", address));
            attributes.push(KeyValue::new("server.port", port));
        }

        if let Some(user_agent) = conn.request_headers().get_str(KnownHeaderName::UserAgent) {
            attributes.push(KeyValue::new("user_agent.original", user_agent.to_string()));
        }

        let name = if let Some(route) = self.route.as_ref().and_then(|route| route(&conn)) {
            conn.set_state(RouteWasAvailable);
            attributes.push(KeyValue::new("http.route", route.clone()));
            format!("{} {route}", conn.method().as_str()).into()
        } else {
            conn.method().as_str().into()
        };

        let span = self.tracer.build(SpanBuilder {
            name,
            start_time,
            span_kind: Some(SpanKind::Server),
            attributes: Some(attributes),
            ..SpanBuilder::default()
        });
        let context = Context::current_with_span(span);

        conn.with_state(TraceContext { context })
    }

    async fn before_send(&self, mut conn: Conn) -> Conn {
        let Some(TraceContext { context }) = conn.state().cloned() else {
            return conn;
        };

        let span = context.span();

        let error_type = self
            .error_type
            .as_ref()
            .and_then(|et| et(&conn))
            .or_else(|| {
                let status = conn.status().unwrap_or(Status::NotFound);
                if status.is_server_error() {
                    Some((status as u16).to_string().into())
                } else {
                    None
                }
            });

        if conn.status().map_or(false, |s| s.is_server_error()) {
            span.set_status(opentelemetry::trace::Status::Error {
                description: "".into(), // see error.type
            });
        }

        let status: i64 = (conn.status().unwrap_or(Status::NotFound) as u16).into();

        let mut attributes = vec![KeyValue::new("http.response.status_code", status)];

        if conn.take_state::<RouteWasAvailable>().is_none() {
            let route = self.route.as_ref().and_then(|route| route(&conn));
            if let Some(route) = &route {
                attributes.push(KeyValue::new("http.route", route.clone()));
                span.update_name(format!("{} {route}", conn.method().as_str()));
            }
        }

        if let Some(error_type) = error_type {
            attributes.push(KeyValue::new("error.type", error_type));
        }

        span.set_attributes(attributes);

        {
            let context = context.clone();
            conn.inner_mut().after_send(move |send_status| {
                let span = context.span();
                if !send_status.is_success() {
                    span.set_status(opentelemetry::trace::Status::Error {
                        description: "http send error".into(),
                    });
                    span.set_attribute(KeyValue::new("error.type", "http send error"));
                }
                span.end();
            });
        }

        conn
    }
}
