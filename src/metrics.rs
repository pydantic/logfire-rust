use std::borrow::Cow;
use std::sync::LazyLock;

use opentelemetry::metrics::{
    AsyncInstrumentBuilder, Counter, Gauge, Histogram, HistogramBuilder, InstrumentBuilder, Meter,
    ObservableCounter, ObservableGauge, ObservableUpDownCounter, UpDownCounter,
};

static METER: LazyLock<Meter> = LazyLock::new(|| opentelemetry::global::meter("logfire"));

/// For methods which are called with an observation.
#[rustfmt::skip]
macro_rules! make_metric_doc {
    ($method: ident, $ty:ty, $var_name:literal, $usage:literal) => {
        concat!(
"Wrapper for [`Meter::", stringify!($method), "`] using logfire's global meter.

# Examples

We recommend using this as a static variable, like so:

```rust
use std::sync::LazyLock;
use opentelemetry::metrics::AsyncInstrument;

static ", $var_name, ": LazyLock<opentelemetry::metrics::", stringify!($ty), "> = LazyLock::new(|| {
    logfire::", stringify!($method), "(\"my_counter\")
        .with_description(\"Just an example\")
        .with_unit(\"s\")
        .build()
});

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ensure logfire is configured before accessing the metric for the first time
    let shutdown_handler = logfire::configure()
#        .send_to_logfire(logfire::config::SendToLogfire::IfTokenPresent)
        .finish()?;

    // send a value to the metric
    ", $usage, ";

    shutdown_handler.shutdown()?;
    Ok(())
}
```
"
        )
    };
}

macro_rules! wrap_method {
    ($method:ident, $ty:ty, $var_name:literal, $usage:literal) => {
        #[doc = make_metric_doc!($method, $ty, $var_name, $usage)]
        pub fn $method(name: impl Into<Cow<'static, str>>) -> InstrumentBuilder<'static, $ty> {
            METER.$method(name)
        }
    };
}

macro_rules! wrap_histogram_method {
    ($method:ident, $ty:ty, $var_name:literal, $usage:literal) => {
        #[doc = make_metric_doc!($method, $ty, $var_name, $usage)]
        pub fn $method(name: impl Into<Cow<'static, str>>) -> HistogramBuilder<'static, $ty> {
            METER.$method(name)
        }
    };
}

wrap_method!(
    f64_counter,
    Counter<f64>,
    "COUNTER",
    "COUNTER.add(1.0, &[])"
);
wrap_method!(f64_gauge, Gauge<f64>, "GAUGE", "GAUGE.record(1.0, &[])");
wrap_method!(
    f64_up_down_counter,
    UpDownCounter<f64>,
    "UP_DOWN_COUNTER",
    "UP_DOWN_COUNTER.add(1.0, &[])"
);
wrap_method!(i64_gauge, Gauge<i64>, "GAUGE", "GAUGE.record(1, &[])");
wrap_method!(
    i64_up_down_counter,
    UpDownCounter<i64>,
    "UP_DOWN_COUNTER",
    "UP_DOWN_COUNTER.add(1, &[])"
);
wrap_method!(u64_counter, Counter<u64>, "COUNTER", "COUNTER.add(1, &[])");
wrap_method!(u64_gauge, Gauge<u64>, "GAUGE", "GAUGE.record(1, &[])");

wrap_histogram_method!(
    f64_histogram,
    Histogram<f64>,
    "HISTOGRAM",
    "HISTOGRAM.record(1.0, &[])"
);
wrap_histogram_method!(
    u64_histogram,
    Histogram<u64>,
    "HISTOGRAM",
    "HISTOGRAM.record(1, &[])"
);

/// For observable methods which take a callback.
#[rustfmt::skip]
macro_rules! make_observable_metric_doc {
    ($method: ident, $ty:ty, $var_name:literal, $callback:literal) => {
        concat!(
"Wrapper for [`Meter::", stringify!($method), "`] using logfire's global meter.

# Examples

We recommend using this as a static variable, like so:

```rust
use std::sync::LazyLock;

static ", $var_name, ": LazyLock<opentelemetry::metrics::", stringify!($ty), "> = LazyLock::new(|| {
    logfire::", stringify!($method), "(\"my_counter\")
        .with_description(\"Just an example\")
        .with_unit(\"s\")
        .with_callback(", $callback, ")
        .build()
});

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ensure logfire is configured before accessing the metric for the first time
    let shutdown_handler = logfire::configure()
#        .send_to_logfire(logfire::config::SendToLogfire::IfTokenPresent)
        .finish()?;

    // initialize the metric
    LazyLock::force(&", $var_name, ");

    // allow some time for the metric to sample
    std::thread::sleep(std::time::Duration::from_secs(1));

    shutdown_handler.shutdown()?;
    Ok(())
}
```
"
        )
    };
}

macro_rules! wrap_observable_method {
    ($method:ident, $ty:ty, $unit:ident, $var_name:literal, $usage:literal) => {
        #[doc = make_observable_metric_doc!($method, $ty, $var_name, $usage)]
        pub fn $method(
            name: impl Into<Cow<'static, str>>,
        ) -> AsyncInstrumentBuilder<'static, $ty, $unit> {
            METER.$method(name)
        }
    };
}

wrap_observable_method!(
    f64_observable_counter,
    ObservableCounter<f64>,
    f64,
    "COUNTER",
    "|counter| counter.observe(1.0, &[])"
);
wrap_observable_method!(
    f64_observable_gauge,
    ObservableGauge<f64>,
    f64,
    "GAUGE",
    "|gauge| gauge.observe(1.0, &[])"
);
wrap_observable_method!(
    f64_observable_up_down_counter,
    ObservableUpDownCounter<f64>,
    f64,
    "UP_DOWN_COUNTER",
    "|counter| counter.observe(1.0, &[])"
);
wrap_observable_method!(
    i64_observable_gauge,
    ObservableGauge<i64>,
    i64,
    "GAUGE",
    "|gauge| gauge.observe(1, &[])"
);
wrap_observable_method!(
    i64_observable_up_down_counter,
    ObservableUpDownCounter<i64>,
    i64,
    "UP_DOWN_COUNTER",
    "|counter| counter.observe(1, &[])"
);
wrap_observable_method!(
    u64_observable_counter,
    ObservableCounter<u64>,
    u64,
    "COUNTER",
    "|counter| counter.observe(1, &[])"
);
wrap_observable_method!(
    u64_observable_gauge,
    ObservableGauge<u64>,
    u64,
    "GAUGE",
    "|gauge| gauge.observe(1, &[])"
);
