use opentelemetry::{global::set_meter_provider, runtime::Tokio};
use opentelemetry_otlp::{new_exporter, new_pipeline};
use trillium_opentelemetry::Metrics;
use trillium_router::{router, RouterConnExt};

fn set_up_collector() {
    set_meter_provider(
        new_pipeline()
            .metrics(Tokio)
            .with_exporter(new_exporter().tonic())
            .build()
            .unwrap(),
    );
}

#[tokio::main]
pub async fn main() {
    set_up_collector();

    trillium_tokio::run_async((
        Metrics::new("example-app").with_route(|conn| conn.route().map(|r| r.to_string())),
        router().get("/some/:path", "ok"),
    ))
    .await;
}
