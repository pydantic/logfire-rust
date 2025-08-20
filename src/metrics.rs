use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

use opentelemetry::metrics::{
    AsyncInstrumentBuilder, Counter, Gauge, Histogram, HistogramBuilder, InstrumentBuilder, Meter,
    ObservableCounter, ObservableGauge, ObservableUpDownCounter, UpDownCounter,
};

static METER: LazyLock<Meter> = LazyLock::new(|| opentelemetry::global::meter("logfire"));
type HistogramName = Cow<'static, str>;
/// A map of histogram name to scale.
///
/// Histograms that are members of this map will be forced to use `Base2ExponentialHistogram`
/// for aggregation by the meter provider view.
///
/// Histogram names are removed from the map when the [`ExponentialHistogram`] is dropped.
pub static EXPONENTIAL_HISTOGRAMS: LazyLock<RwLock<HashMap<HistogramName, i8>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// An instrument that records a distribution of values.
///
/// [`ExponentialHistogram`] can be cloned to create multiple handles to the same instrument. If a [`ExponentialHistogram`] needs to be shared,
/// users are recommended to clone the [`ExponentialHistogram`] instead of creating duplicate [`ExponentialHistogram`]s for the same metric. Creating
/// duplicate [`ExponentialHistogram`]s for the same metric could lower SDK performance.
pub struct ExponentialHistogram<T> {
    inner: Arc<Inner<T>>,
}

struct Inner<T> {
    name: HistogramName,
    histogram: Histogram<T>,
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        let mut histograms = EXPONENTIAL_HISTOGRAMS
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        histograms.remove(&self.name);
    }
}

impl<T> ExponentialHistogram<T> {
    /// Adds an additional value to the distribution.
    pub fn record(&self, value: T, attributes: &[opentelemetry::KeyValue]) {
        self.inner.histogram.record(value, attributes);
    }
}

/// Configuration for building an exponential Histogram.
pub struct ExponentialHistogramBuilder<'a, T> {
    name: HistogramName,
    scale: i8,
    inner: HistogramBuilder<'a, Histogram<T>>,
}

impl<'a, T> ExponentialHistogramBuilder<'a, T> {
    /// Create a new instrument builder
    fn new(name: HistogramName, scale: i8, inner: HistogramBuilder<'a, Histogram<T>>) -> Self {
        Self { name, scale, inner }
    }

    /// Set the description for this instrument
    #[must_use]
    pub fn with_description<S: Into<Cow<'static, str>>>(mut self, description: S) -> Self {
        self.inner = self.inner.with_description(description);
        self
    }

    /// Set the unit for this instrument.
    ///
    /// Unit is case sensitive(`kb` is not the same as `kB`).
    ///
    /// Unit must be:
    /// - ASCII string
    /// - No longer than 63 characters
    #[must_use]
    pub fn with_unit<S: Into<Cow<'static, str>>>(mut self, unit: S) -> Self {
        self.inner = self.inner.with_unit(unit);
        self
    }
}

impl ExponentialHistogramBuilder<'_, u64> {
    /// Creates a new instrument.
    ///
    /// Validates the instrument configuration and creates a new instrument. In
    /// case of invalid configuration, a no-op instrument is returned
    /// and an error is logged using internal logging.
    #[must_use]
    pub fn build(self) -> ExponentialHistogram<u64> {
        {
            let mut histograms = EXPONENTIAL_HISTOGRAMS
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            histograms.insert(self.name.clone(), self.scale);
        }

        let histogram = self.inner.build();

        ExponentialHistogram {
            inner: Arc::new(Inner {
                name: self.name,
                histogram,
            }),
        }
    }
}

impl ExponentialHistogramBuilder<'_, f64> {
    /// Creates a new instrument.
    ///
    /// Validates the instrument configuration and creates a new instrument. In
    /// case of invalid configuration, a no-op instrument is returned
    /// and an error is logged using internal logging.
    #[must_use]
    pub fn build(self) -> ExponentialHistogram<f64> {
        {
            let mut histograms = EXPONENTIAL_HISTOGRAMS
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            histograms.insert(self.name.clone(), self.scale);
        }

        let histogram = self.inner.build();

        ExponentialHistogram {
            inner: Arc::new(Inner {
                name: self.name,
                histogram,
            }),
        }
    }
}

#[rustfmt::skip]
macro_rules! metric_doc_header {
  (histogram, $method:ident) => {
    concat!("Wrapper for [`Meter::", stringify!($method), "`] using Pydantic Logfire's global meter.")
  };
  (exponential_histogram, $method:ident) => {
    concat!("Wrapper for [`Histogram`] using Base2ExponentialHistogram aggregation and Pydantic Logfire's global meter.")
  };
}

macro_rules! metric_doc_create_metric {
    (histogram, $method:ident, $ty:ty, $var_name:literal) => {
        concat!(
            "static ",
            $var_name,
            ": LazyLock<opentelemetry::metrics::",
            stringify!($ty),
            "> = LazyLock::new(|| {
    logfire::",
            stringify!($method),
            "(\"my_counter\")
        .with_description(\"Just an example\")
        .with_unit(\"s\")
        .build()
});"
        )
    };
    (exponential_histogram, $method:ident, $ty:ty, $var_name:literal) => {
        concat!(
            "static ",
            $var_name,
            ": LazyLock<opentelemetry::metrics::",
            stringify!($ty),
            "> = LazyLock::new(|| {
    logfire::",
            stringify!($method),
            "(\"latency\", 20)
        .with_description(\"Just an example\")
        .with_unit(\"ms\")
        .build()
});"
        )
    };
}

/// For methods which are called with an observation.
#[rustfmt::skip]
macro_rules! make_metric_doc {
    ($method: ident, $ty:ty, $var_name:literal, $usage:literal, $histogram_type: ident) => {
        concat!(
metric_doc_header!($histogram_type, $method),
"
# Examples

We recommend using this as a static variable, like so:

```rust
use std::sync::LazyLock;
use opentelemetry::metrics::AsyncInstrument;
",
metric_doc_create_metric!($histogram_type, $method, $ty, $var_name),
"
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ensure Pydantic Logfire is configured before accessing the metric for the first time
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
        #[doc = make_metric_doc!($method, $ty, $var_name, $usage, histogram)]
        pub fn $method(name: impl Into<Cow<'static, str>>) -> InstrumentBuilder<'static, $ty> {
            METER.$method(name)
        }
    };
}

macro_rules! wrap_histogram_method {
    ($method:ident, $ty:ty, $var_name:literal, $usage:literal) => {
        #[doc = make_metric_doc!($method, $ty, $var_name, $usage, histogram)]
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

#[doc = make_metric_doc!(f64_exponential_histogram, ExponentialHistogram<f64>, "HISTOGRAM", "HISTOGRAM.record(1, &[])", exponential_histogram)]
pub fn f64_exponential_histogram(
    name: impl Into<Cow<'static, str>>,
    scale: i8,
) -> ExponentialHistogramBuilder<'static, f64> {
    let name = name.into();
    ExponentialHistogramBuilder::new(name.clone(), scale, f64_histogram(name))
}

#[doc = make_metric_doc!(u64_exponential_histogram, ExponentialHistogram<u64>, "HISTOGRAM", "HISTOGRAM.record(1, &[])", exponential_histogram)]
pub fn u64_exponential_histogram(
    name: impl Into<Cow<'static, str>>,
    scale: i8,
) -> ExponentialHistogramBuilder<'static, u64> {
    let name = name.into();
    ExponentialHistogramBuilder::new(name.clone(), scale, u64_histogram(name))
}

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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn exponential_histograms_are_registered_and_removed_on_drop() {
        let a = f64_exponential_histogram("f64_exp", 10).build();
        let b = u64_exponential_histogram("u64_exp", 20).build();

        // Exponential histograms have their names automatically added to EXPONENTIAL_HISTOGRAMS.
        {
            let histograms = EXPONENTIAL_HISTOGRAMS.read().unwrap();
            assert!(histograms.contains_key("f64_exp"));
            assert!(histograms.contains_key("u64_exp"));
        }

        drop(a);
        drop(b);

        // And have their names removed from the map on drop.
        {
            let histograms = EXPONENTIAL_HISTOGRAMS.read().unwrap();
            assert!(!histograms.contains_key("f64_exp"));
            assert!(!histograms.contains_key("u64_exp"));
        }
    }
}
