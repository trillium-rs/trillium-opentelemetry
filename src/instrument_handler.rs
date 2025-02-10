use crate::{instrumentation_scope, trace::TraceContext};
use opentelemetry::{
    global::BoxedTracer,
    trace::{FutureExt, TraceContextExt, Tracer},
    Context,
};
use trillium::{async_trait, Conn, Handler, Info, Upgrade};

/// Trillium handler that instruments handlers with spans.
///
/// **IMPORTANT** This handler expects [`crate::Trace`] or [`crate::Instrument`] to have been run on
/// the conn prior to running this handler.
#[derive(Debug, Clone)]
pub struct InstrumentHandler<H, T> {
    handler: H,
    tracer: T,
}

#[async_trait]
impl<H, T> Handler for InstrumentHandler<H, T>
where
    H: Handler,
    T: Tracer + Send + Sync + 'static,
    T::Span: Send + Sync + 'static,
{
    async fn init(&mut self, info: &mut Info) {
        let name = self.handler.name();
        self.handler
            .init(info)
            .with_context(Context::current_with_span(
                self.tracer.start(format!("{name}::init")),
            ))
            .await
    }

    async fn run(&self, mut conn: Conn) -> Conn {
        let name = self.handler.name();
        match conn.take_state() {
            Some(TraceContext { context }) => {
                let child = self
                    .tracer
                    .start_with_context(format!("{name}::run"), &context);
                let child_context = Context::current_with_span(child);
                self.handler
                    .run(conn.with_state(TraceContext {
                        context: child_context.clone(),
                    }))
                    .with_context(child_context)
                    .await
                    .with_state(TraceContext { context })
            }

            None => self.handler.run(conn).await,
        }
    }

    async fn before_send(&self, mut conn: Conn) -> Conn {
        let name = self.handler.name();
        match conn.take_state() {
            Some(TraceContext { context }) => {
                let child = self
                    .tracer
                    .start_with_context(format!("{name}::before_send"), &context);

                let child_context = Context::current_with_span(child);
                self.handler
                    .before_send(conn.with_state(TraceContext {
                        context: child_context.clone(),
                    }))
                    .with_context(child_context)
                    .await
                    .with_state(TraceContext { context })
            }

            None => self.handler.before_send(conn).await,
        }
    }

    fn has_upgrade(&self, upgrade: &Upgrade) -> bool {
        self.handler.has_upgrade(upgrade)
    }

    async fn upgrade(&self, upgrade: Upgrade) {
        let name = self.handler.name();
        match upgrade.state().get() {
            Some(TraceContext { context }) => {
                let child = self
                    .tracer
                    .start_with_context(format!("{name}::upgrade"), context);

                self.handler
                    .upgrade(upgrade)
                    .with_context(Context::current_with_span(child))
                    .await
            }

            None => self.handler.upgrade(upgrade).await,
        }
    }
}

/// decorate a handler with a specific tracer
///
/// **IMPORTANT** This handler expects [`crate::Trace`] or [`crate::Instrument`] to have been run on
/// the conn prior to running this handler.
pub fn instrument_handler<H, T>(handler: H, tracer: T) -> InstrumentHandler<H, T>
where
    H: Handler,
    T: Tracer + Send + Sync + 'static,
    T::Span: Send + Sync + 'static,
{
    InstrumentHandler::new(handler, tracer)
}

impl<H, T> InstrumentHandler<H, T>
where
    H: Handler,
    T: Tracer + Send + Sync + 'static,
    T::Span: Send + Sync + 'static,
{
    /// decorate a handler with a specific tracer
    ///
    /// **IMPORTANT** This handler expects [`crate::Trace`] or [`crate::Instrument`] to have been run on
    /// the conn prior to running this handler.
    pub fn new(handler: H, tracer: T) -> Self {
        Self { handler, tracer }
    }
}

/// the primary entrypoint for decorating a handler.
///
/// Uses a global tracer with the name `"trillium-opentelemetry"`
///
/// **IMPORTANT** This handler expects [`crate::Trace`] or [`crate::Instrument`] to have been run on
/// the conn prior to running this handler.
pub fn instrument_handler_global<H>(handler: H) -> InstrumentHandler<H, BoxedTracer>
where
    H: Handler,
{
    InstrumentHandler::new(
        handler,
        opentelemetry::global::tracer_with_scope(instrumentation_scope()),
    )
}
