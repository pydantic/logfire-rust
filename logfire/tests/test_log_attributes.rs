//! Tests for logfire::log! macro attribute forms.

use logfire::{config::AdvancedOptions, log};
use opentelemetry_sdk::logs::{InMemoryLogExporter, SimpleLogProcessor};

#[path = "../src/test_utils.rs"]
mod test_utils;

use test_utils::{find_log, find_log_attr};
use tracing::Level;

#[test]
fn test_log_macro_attributes() {
    let log_exporter = InMemoryLogExporter::default();
    let logfire = logfire::configure()
        .local()
        .send_to_logfire(false)
        .with_advanced_options(
            AdvancedOptions::default()
                .with_log_processor(SimpleLogProcessor::new(log_exporter.clone())),
        )
        .finish()
        .unwrap();
    let guard = logfire::set_local_logfire(logfire);

    let _ = log!(Level::INFO, "string_attr_log", foo = "bar");
    let _ = log!(Level::INFO, "int_attr_log", num = 42);
    let _ = log!(Level::INFO, "bool_attr_log", flag = true);
    let _ = log!(Level::INFO, "dotted_attr_log", dotted.key = "value");
    let _ = log!(
        Level::INFO,
        "multi_attr_log",
        a = 1,
        b = "two",
        c = false,
        d.e = 3
    );

    guard.shutdown().unwrap();
    let logs = log_exporter.get_emitted_logs().unwrap();

    // String attribute
    let log = find_log(&logs, "string_attr_log");
    let value = find_log_attr(log, "foo");
    assert!(matches!(value, opentelemetry::logs::AnyValue::String(s) if s.as_str() == "bar"));

    // Integer attribute
    let log = find_log(&logs, "int_attr_log");
    let value = find_log_attr(log, "num");
    assert!(matches!(value, opentelemetry::logs::AnyValue::Int(v) if *v == 42));

    // Boolean attribute
    let log = find_log(&logs, "bool_attr_log");
    let value = find_log_attr(log, "flag");
    assert!(matches!(value, opentelemetry::logs::AnyValue::Boolean(v) if *v));

    // Dotted key attribute
    let log = find_log(&logs, "dotted_attr_log");
    let value = find_log_attr(log, "dotted.key");
    assert!(matches!(value, opentelemetry::logs::AnyValue::String(s) if s.as_str() == "value"));

    // Multiple attributes
    let log = find_log(&logs, "multi_attr_log");
    let value = find_log_attr(log, "a");
    assert!(matches!(value, opentelemetry::logs::AnyValue::Int(v) if *v == 1));
    let value = find_log_attr(log, "b");
    assert!(matches!(value, opentelemetry::logs::AnyValue::String(s) if s.as_str() == "two"));
    let value = find_log_attr(log, "c");
    assert!(matches!(value, opentelemetry::logs::AnyValue::Boolean(v) if !*v));
    let value = find_log_attr(log, "d.e");
    assert!(matches!(value, opentelemetry::logs::AnyValue::Int(v) if *v == 3));
}

#[test]
fn test_log_macro_shorthand_ident() {
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

    let log_exporter = InMemoryLogExporter::default();
    let logfire = logfire::configure()
        .local()
        .send_to_logfire(false)
        .with_advanced_options(
            AdvancedOptions::default()
                .with_log_processor(SimpleLogProcessor::new(log_exporter.clone())),
        )
        .finish()
        .unwrap();

    let guard = logfire::set_local_logfire(logfire);
    let _ = log!(Level::INFO, "dotted_attr_log", dotted.key);
    let _ = log!(Level::INFO, "int_attr_log", int_val);
    let _ = log!(Level::INFO, "bool_attr_log", bool_val);
    let _ = log!(
        Level::INFO,
        "multi_attr_log",
        multi.a,
        multi.b,
        multi.c,
        multi.d_e
    );

    guard.shutdown().unwrap();

    let logs = log_exporter.get_emitted_logs().unwrap();

    // Dotted key attribute
    let log = find_log(&logs, "dotted_attr_log");
    let value = find_log_attr(log, "dotted.key");
    assert!(matches!(value, opentelemetry::logs::AnyValue::String(s) if s.as_str() == "value"));

    // Int attribute
    let log = find_log(&logs, "int_attr_log");
    let value = find_log_attr(log, "int_val");
    assert!(matches!(value, opentelemetry::logs::AnyValue::Int(v) if *v == 42));

    // Bool attribute
    let log = find_log(&logs, "bool_attr_log");
    let value = find_log_attr(log, "bool_val");
    assert!(matches!(value, opentelemetry::logs::AnyValue::Boolean(v) if *v));

    // Multi attributes
    let log = find_log(&logs, "multi_attr_log");
    let value = find_log_attr(log, "multi.a");
    assert!(matches!(value, opentelemetry::logs::AnyValue::Int(v) if *v == 1));
    let value = find_log_attr(log, "multi.b");
    assert!(matches!(value, opentelemetry::logs::AnyValue::String(s) if s.as_str() == "two"));
    let value = find_log_attr(log, "multi.c");
    assert!(matches!(value, opentelemetry::logs::AnyValue::Boolean(v) if !*v));
    let value = find_log_attr(log, "multi.d_e");
    assert!(matches!(value, opentelemetry::logs::AnyValue::Int(v) if *v == 3));
}
