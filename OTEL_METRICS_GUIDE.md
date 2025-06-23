# Exporting OpenTelemetry Metrics from Rust to Logfire

This guide shows how to export OpenTelemetry metrics from Rust applications to Logfire and use them for creating dashboards and monitoring.

## Overview

The Logfire Rust SDK provides built-in support for OpenTelemetry metrics through its metrics API. Metrics are automatically exported to Logfire alongside traces and logs, giving you comprehensive observability for your Rust applications.

## Prerequisites

1. **Set up a Logfire project** at [logfire.pydantic.dev](https://logfire.pydantic.dev/docs/#logfire)
2. **Create a write token** following the [token creation guide](https://logfire.pydantic.dev/docs/how-to-guides/create-write-tokens/)
3. **Set the environment variable**: `export LOGFIRE_TOKEN=your_token_here`

## Quick Start

## Available Metric Types

The Logfire SDK supports all OpenTelemetry metric types:

### Counters

Monotonically increasing values (e.g., request counts, error counts):

```rust
// For counting discrete events
static HTTP_REQUESTS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    logfire::u64_counter("http_requests_total")
        .with_description("Total HTTP requests")
        .with_unit("{request}")
        .build()
});

// For floating-point measurements
static DATA_PROCESSED: LazyLock<Counter<f64>> = LazyLock::new(|| {
    logfire::f64_counter("data_processed_bytes")
        .with_description("Total bytes processed")
        .with_unit("By")
        .build()
});
```

### Gauges

Current state values that can go up or down:

```rust
static ACTIVE_CONNECTIONS: LazyLock<Gauge<u64>> = LazyLock::new(|| {
    logfire::u64_gauge("active_connections")
        .with_description("Number of active connections")
        .with_unit("{connection}")
        .build()
});

static CPU_USAGE: LazyLock<Gauge<f64>> = LazyLock::new(|| {
    logfire::f64_gauge("cpu_usage_percent")
        .with_description("CPU usage percentage")
        .with_unit("%")
        .build()
});
```

### Histograms

Distribution of values (e.g., request durations, response sizes):

```rust
static REQUEST_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    logfire::f64_histogram("http_request_duration")
        .with_description("HTTP request duration")
        .with_unit("s")
        .build()
});

static RESPONSE_SIZE: LazyLock<Histogram<u64>> = LazyLock::new(|| {
    logfire::u64_histogram("http_response_size")
        .with_description("HTTP response size")
        .with_unit("By")
        .build()
});
```

### Up/Down Counters

Values that can increase or decrease:

```rust
static QUEUE_SIZE: LazyLock<UpDownCounter<i64>> = LazyLock::new(|| {
    logfire::i64_up_down_counter("queue_size")
        .with_description("Current queue size")
        .with_unit("{item}")
        .build()
});
```

## Web Framework Integration

### Axum Example

Here's a complete example showing metrics integration with Axum:

```rust
use axum::{Json, Router, extract::Path, http::StatusCode, routing::{get, post}};
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use axum_otel_metrics::HttpMetricsLayerBuilder;
use opentelemetry::{KeyValue, metrics::Counter};
use std::sync::LazyLock;
use tokio::net::TcpListener;

// Custom application metrics
static REQUEST_COUNTER: LazyLock<Counter<u64>> = LazyLock::new(|| {
    logfire::u64_counter("http_requests_total")
        .with_description("Total number of HTTP requests")
        .with_unit("{request}")
        .build()
});

static USER_OPERATIONS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    logfire::u64_counter("user_operations_total")
        .with_description("Total user operations")
        .with_unit("{operation}")
        .build()
});

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Logfire
    let shutdown_handler = logfire::configure()
        .install_panic_handler()
        .finish()?;

    let app = Router::new()
        .route("/users/{id}", get(get_user))
        .route("/users", post(create_user))
        // OpenTelemetry tracing middleware
        .layer(OtelAxumLayer::default())
        .layer(OtelInResponseLayer::default())
        // Automatic HTTP metrics (request duration, response size, etc.)
        .layer(HttpMetricsLayerBuilder::new().build())
        .layer(axum::middleware::from_fn(custom_metrics_middleware));

    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;

    shutdown_handler.shutdown()?;
    Ok(())
}

async fn get_user(Path(user_id): Path<u32>) -> Result<Json<User>, StatusCode> {
    // Record user operation
    USER_OPERATIONS.add(1, &[
        KeyValue::new("operation", "get"),
        KeyValue::new("user_id", user_id as i64),
    ]);

    // Your business logic here...
    Ok(Json(User { id: user_id, name: format!("User {}", user_id) }))
}

// Custom middleware for additional metrics
async fn custom_metrics_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = request.method().clone();
    let uri = request.uri().clone();

    let response = next.run(request).await;

    // Custom request counter with labels
    REQUEST_COUNTER.add(1, &[
        KeyValue::new("method", method.to_string()),
        KeyValue::new("status_code", response.status().as_u16() as i64),
        KeyValue::new("route", uri.path().to_string()),
    ]);

    response
}

#[derive(serde::Serialize)]
struct User {
    id: u32,
    name: String,
}
```

### Actix Web Example

Similar integration is available for Actix Web:

```rust
use actix_web::{App, HttpServer, web, HttpResponse};
use opentelemetry_instrumentation_actix_web::{RequestMetrics, RequestTracing};
use opentelemetry::{KeyValue, metrics::Counter};
use std::sync::LazyLock;

static API_CALLS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    logfire::u64_counter("api_calls_total")
        .with_description("Total API calls")
        .with_unit("{call}")
        .build()
});

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let shutdown_handler = logfire::configure()
        .install_panic_handler()
        .finish()?;

    HttpServer::new(|| {
        App::new()
            // OpenTelemetry middleware for automatic tracing and metrics
            .wrap(RequestTracing::new())
            .wrap(RequestMetrics::default())
            .route("/api/health", web::get().to(health_check))
    })
    .bind("127.0.0.1:3000")?
    .run()
    .await?;

    shutdown_handler.shutdown()?;
    Ok(())
}

async fn health_check() -> HttpResponse {
    API_CALLS.add(1, &[KeyValue::new("endpoint", "health")]);
    HttpResponse::Ok().json(serde_json::json!({"status": "healthy"}))
}
```

## Observable Metrics

For metrics that need to be sampled periodically rather than recorded on-demand:

```rust
use std::sync::LazyLock;
use opentelemetry::metrics::ObservableGauge;

static MEMORY_USAGE: LazyLock<ObservableGauge<u64>> = LazyLock::new(|| {
    logfire::u64_observable_gauge("memory_usage_bytes")
        .with_description("Current memory usage")
        .with_unit("By")
        .with_callback(|observer| {
            // Get current memory usage (example)
            let memory_usage = get_memory_usage();
            observer.observe(memory_usage, &[]);
        })
        .build()
});

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let shutdown_handler = logfire::configure().finish()?;

    // Initialize the observable metric
    LazyLock::force(&MEMORY_USAGE);

    // Keep the application running to allow periodic observations
    std::thread::sleep(std::time::Duration::from_secs(60));

    shutdown_handler.shutdown()?;
    Ok(())
}

fn get_memory_usage() -> u64 {
    // Implementation to get actual memory usage
    1024 * 1024 * 100 // Example: 100MB
}
```

## Using Metrics in Logfire Dashboards

Once your metrics are being exported to Logfire, you can create dashboards and alerts:

### 1. **View Metrics in Logfire**

- Navigate to your Logfire project dashboard
- Go to the ["Explore"](https://logfire.pydantic.dev/docs/guides/web-ui/explore/) view
- Run `select * from metrics` to view all incoming metrics from your application

## Best Practices

1. **Use Static Variables**: Define metrics as static variables with `LazyLock` for efficiency
2. **Meaningful Names**: Use descriptive metric names following OpenTelemetry conventions
3. **Consistent Labels**: Use consistent label names across related metrics
4. **Appropriate Units**: Specify units (seconds, bytes, requests) for better dashboard formatting
5. **Documentation**: Add descriptions to help with dashboard creation
6. **Label Cardinality**: Be careful with high-cardinality labels (e.g., user IDs) - prefer aggregated metrics

## Troubleshooting

### Metrics Not Appearing

- Verify `LOGFIRE_TOKEN` is set correctly
- Check that metrics are being recorded (add debug logging)
- Ensure the application runs long enough for metrics to be exported

### Performance Issues

- Monitor label cardinality - too many unique label combinations can impact performance
- Use histograms instead of gauges for distributions
- Consider sampling for high-frequency metrics

### Dashboard Issues

- Check metric names match exactly (case-sensitive)
- Verify time ranges in queries
- Use Logfire's query builder to test metric availability

This guide provides a complete foundation for implementing metrics in your Rust applications and visualizing them in Logfire dashboards.
