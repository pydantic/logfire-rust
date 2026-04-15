//! Tests for setting resource attributes.
//!
//! In separate tests because they modify environment variables so we can test the
//! interaction with the OTEL sdk.

use insta::assert_debug_snapshot;
use logfire::{
    config::{AdvancedOptions, LogfireConfigBuilder},
    configure,
};
use opentelemetry::KeyValue;
use opentelemetry_sdk::logs::{InMemoryLogExporter, SimpleLogProcessor};

use crate::test_utils::make_deterministic_resource;

#[path = "../src/test_utils.rs"]
mod test_utils;

/// Mutext to ensure tests that modify env vars are not run in parallel.
static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn try_get_resource_attrs(config: LogfireConfigBuilder, env: &[(&str, &str)]) -> Vec<KeyValue> {
    let _lock = TEST_MUTEX.lock().unwrap();

    for env in env {
        // SAFETY: running in separate test with mutex lock
        unsafe {
            std::env::set_var(env.0, env.1);
        }
    }

    let exporter = InMemoryLogExporter::default();

    let logfire = config
        .local()
        .send_to_logfire(false)
        .with_advanced_options(
            AdvancedOptions::default()
                .with_log_processor(SimpleLogProcessor::new(exporter.clone())),
        )
        .finish()
        .expect("failed to configure logfire");

    let guard = logfire::set_local_logfire(logfire);

    logfire::info!("test span");

    guard.shutdown().expect("shutdown should succeed");

    let mut logs = exporter.get_emitted_logs().unwrap();

    assert_eq!(logs.len(), 1);
    let log = logs.pop().unwrap();

    for env in env {
        // SAFETY: running in separate test with mutex lock
        unsafe {
            std::env::remove_var(env.0);
        }
    }

    make_deterministic_resource(&log.resource)
}

#[test]
fn test_no_service_resource_attributes() {
    let attrs = try_get_resource_attrs(configure(), &[]);

    assert_debug_snapshot!(attrs, @r#"
        [
            KeyValue {
                key: Static(
                    "service.name",
                ),
                value: String(
                    Static(
                        "unknown_service",
                    ),
                ),
            },
            KeyValue {
                key: Static(
                    "telemetry.sdk.language",
                ),
                value: String(
                    Static(
                        "rust",
                    ),
                ),
            },
            KeyValue {
                key: Static(
                    "telemetry.sdk.name",
                ),
                value: String(
                    Static(
                        "opentelemetry",
                    ),
                ),
            },
            KeyValue {
                key: Static(
                    "telemetry.sdk.version",
                ),
                value: String(
                    Static(
                        "0.0.0",
                    ),
                ),
            },
        ]
        "#);
}

#[test]
fn test_service_resource_attributes() {
    let attrs = try_get_resource_attrs(
        configure()
            .with_service_name("test-service")
            .with_service_version("1.2.3")
            .with_environment("testing"),
        &[("OTEL_RESOURCE_ATTRIBUTES", "key1=val1,key2=val2")],
    );

    assert_debug_snapshot!(attrs, @r#"
    [
        KeyValue {
            key: Static(
                "deployment.environment.name",
            ),
            value: String(
                Owned(
                    "testing",
                ),
            ),
        },
        KeyValue {
            key: Owned(
                "key1",
            ),
            value: String(
                Owned(
                    "val1",
                ),
            ),
        },
        KeyValue {
            key: Owned(
                "key2",
            ),
            value: String(
                Owned(
                    "val2",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "service.name",
            ),
            value: String(
                Owned(
                    "test-service",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "service.version",
            ),
            value: String(
                Owned(
                    "1.2.3",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "telemetry.sdk.language",
            ),
            value: String(
                Static(
                    "rust",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "telemetry.sdk.name",
            ),
            value: String(
                Static(
                    "opentelemetry",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "telemetry.sdk.version",
            ),
            value: String(
                Static(
                    "0.0.0",
                ),
            ),
        },
    ]
    "#);
}

#[test]
fn test_service_resource_attributes_from_env() {
    let attrs = try_get_resource_attrs(
        configure(),
        &[
            ("LOGFIRE_SERVICE_NAME", "env-service"),
            ("LOGFIRE_SERVICE_VERSION", "4.5.6"),
            ("LOGFIRE_ENVIRONMENT", "env-testing"),
            // otel service vars should be ignored if logfire vars are present
            ("OTEL_SERVICE_NAME", "otel-service"),
            ("OTEL_SERVICE_VERSION", "7.8.9"),
            // these should still apply
            ("OTEL_RESOURCE_ATTRIBUTES", "key1=val1,key2=val2"),
        ],
    );

    assert_debug_snapshot!(attrs, @r#"
    [
        KeyValue {
            key: Static(
                "deployment.environment.name",
            ),
            value: String(
                Owned(
                    "env-testing",
                ),
            ),
        },
        KeyValue {
            key: Owned(
                "key1",
            ),
            value: String(
                Owned(
                    "val1",
                ),
            ),
        },
        KeyValue {
            key: Owned(
                "key2",
            ),
            value: String(
                Owned(
                    "val2",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "service.name",
            ),
            value: String(
                Owned(
                    "env-service",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "service.version",
            ),
            value: String(
                Owned(
                    "4.5.6",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "telemetry.sdk.language",
            ),
            value: String(
                Static(
                    "rust",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "telemetry.sdk.name",
            ),
            value: String(
                Static(
                    "opentelemetry",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "telemetry.sdk.version",
            ),
            value: String(
                Static(
                    "0.0.0",
                ),
            ),
        },
    ]
    "#);
}

#[test]
fn test_service_resource_attributes_from_otel_env() {
    let attrs = try_get_resource_attrs(
        configure(),
        &[
            ("OTEL_SERVICE_NAME", "otel-service"),
            ("OTEL_SERVICE_VERSION", "7.8.9"),
            ("OTEL_RESOURCE_ATTRIBUTES", "key1=val1,key2=val2"),
        ],
    );

    assert_debug_snapshot!(attrs, @r#"
    [
        KeyValue {
            key: Owned(
                "key1",
            ),
            value: String(
                Owned(
                    "val1",
                ),
            ),
        },
        KeyValue {
            key: Owned(
                "key2",
            ),
            value: String(
                Owned(
                    "val2",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "service.name",
            ),
            value: String(
                Owned(
                    "otel-service",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "service.version",
            ),
            value: String(
                Owned(
                    "7.8.9",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "telemetry.sdk.language",
            ),
            value: String(
                Static(
                    "rust",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "telemetry.sdk.name",
            ),
            value: String(
                Static(
                    "opentelemetry",
                ),
            ),
        },
        KeyValue {
            key: Static(
                "telemetry.sdk.version",
            ),
            value: String(
                Static(
                    "0.0.0",
                ),
            ),
        },
    ]
    "#);
}
