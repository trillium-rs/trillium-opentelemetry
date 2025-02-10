use opentelemetry::{
    global::{set_meter_provider, set_tracer_provider},
    KeyValue,
};
use opentelemetry_otlp::{MetricExporter, SpanExporter};
use opentelemetry_sdk::{
    metrics::{PeriodicReader, SdkMeterProvider},
    trace::SdkTracerProvider,
    Resource,
};
use trillium::{KnownHeaderName, Status};
use trillium_opentelemetry::global::{instrument, instrument_handler};
use trillium_router::{router, RouterConnExt};

#[tokio::main]
pub async fn main() {
    env_logger::init();

    let exporter = MetricExporter::builder().with_http().build().unwrap();
    let reader = PeriodicReader::builder(exporter).build();
    let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
    set_meter_provider(meter_provider);

    let exporter = SpanExporter::builder().with_http().build().unwrap();
    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(
            Resource::builder_empty()
                .with_attribute(KeyValue::new(
                    "service.name",
                    "trillium-opentelemetry/examples/with_global",
                ))
                .build(),
        )
        .with_batch_exporter(exporter)
        .build();
    set_tracer_provider(tracer_provider);

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
