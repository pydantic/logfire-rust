use std::borrow::Cow;
use std::sync::LazyLock;

use opentelemetry::metrics::{
    AsyncInstrumentBuilder, Counter, Gauge, Histogram, HistogramBuilder, InstrumentBuilder, Meter,
    ObservableCounter, ObservableGauge, ObservableUpDownCounter, UpDownCounter,
};

static METER: LazyLock<Meter> = LazyLock::new(|| opentelemetry::global::meter("logfire"));

macro_rules! wrap_method {
    ($method:ident, $ty:ty) => {
        pub fn $method(name: impl Into<Cow<'static, str>>) -> InstrumentBuilder<'static, $ty> {
            METER.$method(name)
        }
    };
}

macro_rules! wrap_histogram_method {
    ($method:ident, $ty:ty) => {
        pub fn $method(name: impl Into<Cow<'static, str>>) -> HistogramBuilder<'static, $ty> {
            METER.$method(name)
        }
    };
}

macro_rules! wrap_async_method {
    ($method:ident, $ty:ty, $unit:ty) => {
        pub fn $method(
            name: impl Into<Cow<'static, str>>,
        ) -> AsyncInstrumentBuilder<'static, $ty, $unit> {
            METER.$method(name)
        }
    };
}

wrap_method!(f64_counter, Counter<f64>);
wrap_method!(f64_gauge, Gauge<f64>);
wrap_method!(f64_up_down_counter, UpDownCounter<f64>);
wrap_method!(i64_gauge, Gauge<i64>);
wrap_method!(i64_up_down_counter, UpDownCounter<i64>);
wrap_method!(u64_counter, Counter<u64>);
wrap_method!(u64_gauge, Gauge<u64>);

wrap_histogram_method!(f64_histogram, Histogram<f64>);
wrap_histogram_method!(u64_histogram, Histogram<u64>);

wrap_async_method!(f64_observable_counter, ObservableCounter<f64>, f64);
wrap_async_method!(f64_observable_gauge, ObservableGauge<f64>, f64);
wrap_async_method!(
    f64_observable_up_down_counter,
    ObservableUpDownCounter<f64>,
    f64
);
wrap_async_method!(i64_observable_gauge, ObservableGauge<i64>, i64);
wrap_async_method!(
    i64_observable_up_down_counter,
    ObservableUpDownCounter<i64>,
    i64
);
wrap_async_method!(u64_observable_counter, ObservableCounter<u64>, u64);
wrap_async_method!(u64_observable_gauge, ObservableGauge<u64>, u64);
