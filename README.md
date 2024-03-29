# Trillium Opentelemetry!

This crate provides opentelemetry metrics conforming to [semantic conventions for http][http-metrics] and [tracing semantic conventions][http-spans].

## Usage:

```rust
use trillium_opentelemetry::global::{instrument, instrument_handler};
use trillium_router::router;

#[tokio::main]
async fn main() {
    // configure a global meter provider and tracer provider here
    // see examples/with_global.rs for a functional example

    trillium_tokio::run_async((
        instrument().with_route(|conn| conn.route().map(|r| r.to_string().into())),
        instrument_handler(router().get("/some/:path", instrument_handler("ok")),
    ))
    .await;
}
```


[http-metrics]: https://opentelemetry.io/docs/specs/semconv/http/http-metrics/
[http-spans]: https://opentelemetry.io/docs/specs/semconv/http/http-spans/

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
