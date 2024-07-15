use opentelemetry::{
    global::{set_meter_provider, set_tracer_provider},
    KeyValue,
};
use opentelemetry_otlp::{new_exporter, new_pipeline};
use opentelemetry_sdk::{runtime::Tokio, trace::Config, Resource};
use trillium::{KnownHeaderName, Status};
use trillium_opentelemetry::global::{instrument, instrument_handler};
use trillium_router::{router, RouterConnExt};

#[tokio::main]
pub async fn main() {
    env_logger::init();
    set_meter_provider(
        new_pipeline()
            .metrics(Tokio)
            .with_exporter(new_exporter().tonic())
            .build()
            .unwrap(),
    );

    set_tracer_provider(
        new_pipeline()
            .tracing()
            .with_trace_config(
                Config::default().with_resource(Resource::new(vec![KeyValue::new(
                    "service.name",
                    "trillium-opentelemetry/examples/with_global",
                )])),
            )
            .with_exporter(new_exporter().tonic())
            .install_batch(Tokio)
            .unwrap(),
    );

    trillium_tokio::run_async((
        instrument()
            .with_headers([KnownHeaderName::Accept])
            .with_route(|conn| conn.route().map(|r| r.to_string().into())),
        instrument_handler(
            router()
                .get("/some/:path", instrument_handler("ok"))
                .get("/error", instrument_handler(Status::InternalServerError)),
        ),
    ))
    .await;
}
