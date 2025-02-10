use opentelemetry::global::set_meter_provider;
use opentelemetry_otlp::MetricExporter;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use trillium_opentelemetry::Metrics;
use trillium_router::{router, RouterConnExt};

fn set_up_collector() {
    let exporter = MetricExporter::builder().with_http().build().unwrap();
    let reader = PeriodicReader::builder(exporter).build();
    let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
    set_meter_provider(meter_provider);
}

#[tokio::main]
pub async fn main() {
    set_up_collector();

    trillium_tokio::run_async((
        Metrics::new("example-app").with_route(|conn| conn.route().map(|r| r.to_string().into())),
        router().get("/some/:path", "ok"),
    ))
    .await;
}
