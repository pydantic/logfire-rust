//! Tests for logfire::span! macro attribute forms.

use logfire::span;
use opentelemetry_sdk::trace::InMemorySpanExporterBuilder;

#[path = "../src/test_utils.rs"]
mod test_utils;

use test_utils::{find_attr, find_span};

#[test]
fn test_span_macro_attributes() {
    let exporter = InMemorySpanExporterBuilder::new().build();
    let logfire = logfire::configure()
        .local()
        .send_to_logfire(false)
        .with_additional_span_processor(opentelemetry_sdk::trace::SimpleSpanProcessor::new(
            exporter.clone(),
        ))
        .finish()
        .unwrap();
    let guard = logfire::set_local_logfire(logfire);

    tracing::subscriber::with_default(guard.subscriber(), || {
        let _ = span!("string_attr_span", foo = "bar");
        let _ = span!("int_attr_span", num = 42);
        let _ = span!("bool_attr_span", flag = true);
        let _ = span!("dotted_attr_span", dotted.key = "value");
        let _ = span!("multi_attr_span", a = 1, b = "two", c = false, d.e = 3);
    });
    let spans = exporter.get_finished_spans().unwrap();

    // String attribute
    let span = find_span(&spans, "string_attr_span");
    let kv = find_attr(span, "foo");
    assert!(matches!(&kv.value, opentelemetry::Value::String(s) if s.as_str() == "bar"));

    // Integer attribute
    let span = find_span(&spans, "int_attr_span");
    let kv = find_attr(span, "num");
    assert!(matches!(&kv.value, opentelemetry::Value::I64(v) if *v == 42));

    // Boolean attribute
    let span = find_span(&spans, "bool_attr_span");
    let kv = find_attr(span, "flag");
    assert!(matches!(&kv.value, opentelemetry::Value::Bool(v) if *v));

    // Dotted key attribute
    let span = find_span(&spans, "dotted_attr_span");
    let kv = find_attr(span, "dotted.key");
    assert!(matches!(&kv.value, opentelemetry::Value::String(s) if s.as_str() == "value"));

    // Multiple attributes
    let span = find_span(&spans, "multi_attr_span");
    let kv = find_attr(span, "a");
    assert!(matches!(&kv.value, opentelemetry::Value::I64(v) if *v == 1));
    let kv = find_attr(span, "b");
    assert!(matches!(&kv.value, opentelemetry::Value::String(s) if s.as_str() == "two"));
    let kv = find_attr(span, "c");
    assert!(matches!(&kv.value, opentelemetry::Value::Bool(v) if !*v));
    let kv = find_attr(span, "d.e");
    assert!(matches!(&kv.value, opentelemetry::Value::I64(v) if *v == 3));
}

#[test]
fn test_span_macro_shorthand_ident() {
    #[derive(Debug)]
    struct Dotted {
        key: &'static str,
    }
    #[derive(Debug)]
    struct Multi {
        a: i64,
        b: &'static str,
        c: bool,
        d_e: i64,
    }

    let dotted = Dotted { key: "value" };
    let int_val = 42;
    let bool_val = true;
    let multi = Multi {
        a: 1,
        b: "two",
        c: false,
        d_e: 3,
    };

    let exporter = InMemorySpanExporterBuilder::new().build();
    let logfire = logfire::configure()
        .local()
        .send_to_logfire(false)
        .with_additional_span_processor(opentelemetry_sdk::trace::SimpleSpanProcessor::new(
            exporter.clone(),
        ))
        .finish()
        .unwrap();
    let guard = logfire::set_local_logfire(logfire);
    tracing::subscriber::with_default(guard.subscriber(), || {
        let _ = span!("dotted_attr_span", dotted.key);
        let _ = span!("int_attr_span", int_val);
        let _ = span!("bool_attr_span", bool_val);
        let _ = span!("multi_attr_span", multi.a, multi.b, multi.c, multi.d_e);
    });

    let spans = exporter.get_finished_spans().unwrap();

    // Dotted key attribute
    let span = find_span(&spans, "dotted_attr_span");
    let kv = find_attr(span, "dotted.key");
    assert!(matches!(&kv.value, opentelemetry::Value::String(s) if s.as_str() == "value"));

    // Int attribute
    let span = find_span(&spans, "int_attr_span");
    let kv = find_attr(span, "int_val");
    assert!(matches!(&kv.value, opentelemetry::Value::I64(v) if *v == 42));

    // Bool attribute
    let span = find_span(&spans, "bool_attr_span");
    let kv = find_attr(span, "bool_val");
    assert!(matches!(&kv.value, opentelemetry::Value::Bool(v) if *v));

    // Multi attributes
    let span = find_span(&spans, "multi_attr_span");
    let kv = find_attr(span, "multi.a");
    assert!(matches!(&kv.value, opentelemetry::Value::I64(v) if *v == 1));
    let kv = find_attr(span, "multi.b");
    assert!(matches!(&kv.value, opentelemetry::Value::String(s) if s.as_str() == "two"));
    let kv = find_attr(span, "multi.c");
    assert!(matches!(&kv.value, opentelemetry::Value::Bool(v) if !*v));
    let kv = find_attr(span, "multi.d_e");
    assert!(matches!(&kv.value, opentelemetry::Value::I64(v) if *v == 3));
}
