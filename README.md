# Trillium Opentelemetry!

This crate provides opentelemetry metrics conforming to [semantic conventions for http](https://opentelemetry.io/docs/reference/specification/metrics/semantic_conventions/http-metrics/). Eventually it may also include support for [tracing semantic conventions](https://opentelemetry.io/docs/reference/specification/trace/semantic_conventions/http/).

## Usage:

```
use trillium_opentelemetry::metrics;
use trillium_router::{router, RouterConnExt};

#[tokio::main]
async fn main() {
    /// configure your meter provider / exporter here

    trillium_tokio::run_async((
        metrics("example-app").with_route(|conn| conn.route().map(|r| r.to_string())),
        router().get("/some/:path", "ok"),
    ))
    .await;
}
```


<br/><hr/><br/>
Legal:

Licensed under either of
 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)</sup>
   
at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
