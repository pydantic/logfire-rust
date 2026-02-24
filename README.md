# logfire-client

A Rust client for [Pydantic Logfire](https://pydantic.dev/logfire), the observability platform for Python applications. Query your logs, traces, and metrics using SQL.

## Installation

```toml
[dependencies]
logfire-client = "0.1"
```

## Usage

### Querying Logs

The `records` table contains all spans and logs.

```rust
use logfire_client::LogfireClientBuilder;
use serde::Deserialize;

#[derive(Deserialize)]
struct ExceptionRecord {
    start_timestamp: String,
    message: String,
    exception_type: Option<String>,
    exception_message: Option<String>,
}

let client = LogfireClientBuilder::from_env()?.build_client()?;

// Use serde_json::Value for ad-hoc queries without defining a struct
let rows = client
    .query::<serde_json::Value>("SELECT * FROM records LIMIT 5")
    .await?;

// Or define a struct for typed access
let exceptions = client
    .query::<ExceptionRecord>(
        "SELECT start_timestamp, message, exception_type, exception_message
         FROM records
         WHERE is_exception
           AND service_name = 'api-server'
           AND start_timestamp > now() - interval '24 hours'
         ORDER BY start_timestamp DESC",
    )
    .await?;
```

### Querying Metrics

The `metrics` table stores time-series data.

```rust
#[derive(Deserialize)]
struct LatencyStats {
    endpoint: String,
    avg_latency_ms: f64,
    p95: f64,
}

let latency = client
    .query::<LatencyStats>(
        "SELECT attributes->>'http.route' AS endpoint,
                avg(scalar_value) AS avg_latency_ms,
                percentile_cont(0.95) WITHIN GROUP (ORDER BY scalar_value) AS p95
         FROM metrics
         WHERE metric_name = 'http.server.duration'
           AND recorded_timestamp > now() - interval '1 hour'
         GROUP BY endpoint
         ORDER BY avg_latency_ms DESC",
    )
    .await?;
```

## sqlx Integration

The examples above use serde to deserialize JSON responses. This works, but the SQL query is just a string - typos in column names or type mismatches only surface at runtime.

With sqlx, queries are checked at compile time:

```toml
[dependencies]
logfire-client = { version = "0.1", features = ["sqlx"] }
```

```rust
use logfire_client::LogfireClientBuilder;

// The builder can be reused
let builder = LogfireClientBuilder::from_env()?;
let pool = builder.build_sqlx_pool().await?;

// This query is verified at compile time:
// - Column names are checked against the schema
// - Return types match the struct fields
// - SQL syntax is validated
let records = sqlx::query_as!(
    ExceptionRecord,
    "SELECT start_timestamp, message, exception_type, exception_message
     FROM records
     WHERE service_name = $1
     LIMIT $2",
    "api-server",
    100i64
)
.fetch_all(&pool)
.await?;
```

If you rename a column or misspell `exception_type`, the compiler catches it before your code runs.

## Configuration

Set `LOGFIRE_READ_TOKEN` to your read token. The base URL is auto-detected from the token's region (`us`, `eu`, etc.).

Read tokens are created in the Logfire web UI under Settings > Read tokens.

```rust
// Reads token from LOGFIRE_READ_TOKEN
let builder = LogfireClientBuilder::from_env()?;

// Or configure explicitly (base_url is optional, detected from token)
let builder = LogfireClientBuilder::new()
    .token("pylf_v1_us_...");
```

## License

MIT
