use opentelemetry::Value;
use opentelemetry_sdk::trace::SpanData;

use crate::internal::constants::ATTRIBUTES_SPAN_TYPE_KEY;

pub trait SpanDataExt {
    fn get_span_type(&self) -> Option<&str>;
}

impl SpanDataExt for SpanData {
    fn get_span_type(&self) -> Option<&str> {
        self.attributes.iter().find_map(|attr| {
            if attr.key.as_str() != ATTRIBUTES_SPAN_TYPE_KEY {
                return None;
            }
            let Value::String(s) = &attr.value else {
                return None;
            };
            Some(s.as_str())
        })
    }
}
