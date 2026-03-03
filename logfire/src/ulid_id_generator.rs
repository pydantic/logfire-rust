use std::cell::RefCell;

use rand::{Rng, RngCore, SeedableRng, rngs};

use opentelemetry::trace::{SpanId, TraceId};
use opentelemetry_sdk::trace::IdGenerator;

const MAX_TIMESTAMP_IN_6_BYTES: u128 = 1 << 48; // 2^(8 * 6)

/// Default [`IdGenerator`] implementation.
///
/// Generates Trace ids following the ULID spec so that they are sortable by time
/// and Span ids using a random number generator.
#[derive(Clone, Debug, Default)]
pub struct UlidIdGenerator {
    _private: (), // no public constructor
}

impl UlidIdGenerator {
    /// Create a new `UlidIdGenerator`
    pub fn new() -> Self {
        UlidIdGenerator { _private: () }
    }
}

impl IdGenerator for UlidIdGenerator {
    fn new_trace_id(&self) -> TraceId {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time went backwards")
            .as_millis();

        assert!(now < MAX_TIMESTAMP_IN_6_BYTES, "Timestamp is too large");

        let mut out = [0u8; 16];
        let (left, right) = out.split_at_mut(6);

        left.copy_from_slice(&now.to_be_bytes()[10..]);

        CURRENT_RNG.with(|rng| rng.borrow_mut().fill_bytes(right));

        TraceId::from_bytes(out)
    }

    fn new_span_id(&self) -> SpanId {
        CURRENT_RNG.with(|rng| SpanId::from(rng.borrow_mut().random::<u64>()))
    }
}

thread_local! {
    /// Store random number generator for each thread
    static CURRENT_RNG: RefCell<rngs::SmallRng> = RefCell::new(rngs::SmallRng::from_rng(&mut rand::rng()));
}

#[cfg(test)]
mod tests {
    use ulid::Ulid;

    use super::*;

    #[test]
    fn test_ulid_id_generator() {
        let generator = UlidIdGenerator::new();
        let trace_id = generator.new_trace_id();
        let span_id = generator.new_span_id();

        assert_eq!(trace_id.to_bytes().len(), 16);
        assert_eq!(span_id.to_bytes().len(), 8);
    }

    #[test]
    fn test_timestamp_from_trace_id() {
        let generator = UlidIdGenerator::new();
        let trace_id = generator.new_trace_id();
        let b = trace_id.to_bytes();

        let timestamp = u128::from_be_bytes([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, b[0], b[1], b[2], b[3], b[4], b[5],
        ]);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time went backwards")
            .as_millis();

        assert!(timestamp <= now && timestamp > now - 1000);

        // Sanity check that this is a valid ULID
        // We don't actually promise this at all but it's a nice property to have internally
        let u = Ulid::from_bytes(b);
        assert_eq!(u128::from(u.timestamp_ms()), timestamp);
    }
}
