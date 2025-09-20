//! Example of using `logfire` to instrument an `axum` webserver
//!
//! This example shows how to:
//! - Set up Logfire with Axum
//! - Instrument HTTP requests with tracing middleware
//! - Log custom events within route handlers
//! - Create spans for business logic
//! - Track metrics for request counts
//!
//! Run with: `cargo run --example axum`
//! Make sure to set a write token as an environment variable (`LOGFIRE_TOKEN`)
//! <https://logfire.pydantic.dev/docs/how-to-guides/create-write-tokens>/

use axum::{
    Json, Router,
    extract::Path,
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
// `axum-tracing-opentelemetry` provides out-of-the-box middleware for connecting axum
// to the `tracing-opentelemetry` sdk upon which logfire is built.
//
// We recommend enabling the `tracing_level_info` feature to be able to see the request spans.
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
// `axum-otel-metrics` provides a metrics layer for axum which directly sends metrics
// to the opentelemetry SDK (which logfire configures for you)
//
// This sets up standard metrics such as `http.request.duration`; these can be seen at
// https://github.com/open-telemetry/semantic-conventions/blob/main/docs/http/http-metrics.md
use axum_otel_metrics::HttpMetricsLayerBuilder;
use opentelemetry::{KeyValue, metrics::Counter};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use tokio::net::TcpListener;
use tracing::Instrument;

static REQUEST_COUNTER: LazyLock<Counter<u64>> = LazyLock::new(|| {
    logfire::u64_counter("http_requests_total")
        .with_description("Total number of HTTP requests")
        .with_unit("{request}")
        .build()
});

#[derive(Serialize, Deserialize)]
struct User {
    id: u32,
    name: String,
    email: String,
}

#[derive(Deserialize)]
struct CreateUserRequest {
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Logfire
    let logfire = logfire::configure().finish()?;
    let _guard = logfire.shutdown_guard();

    logfire::info!("Starting Axum server with Logfire integration");

    // Build the router with middleware
    let app = create_app();

    // Start the server
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    logfire::info!("Server listening on http://127.0.0.1:3000");

    axum::serve(listener, app).await?;

    // Shutdown handled automatically by the guard

    Ok(())
}

fn create_app() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/users/{id}", get(get_user))
        .route("/users", post(create_user))
        .route("/health", get(health_check))
        // Add tracing middleware for automatic request tracing
        .layer(OtelAxumLayer::default().filter(|path| !path.starts_with("/health")))
        .layer(OtelInResponseLayer::default())
        .layer(HttpMetricsLayerBuilder::new().build())
        .layer(middleware::from_fn(metrics_middleware))
}

async fn root() -> &'static str {
    async {
        logfire::info!("Root endpoint accessed");
        "Hello, Axum with Logfire!"
    }
    .instrument(logfire::span!("Handling root request"))
    .await
}

async fn get_user(Path(user_id): Path<u32>) -> Result<Json<User>, StatusCode> {
    async {
        logfire::info!("Fetching user with ID: {user_id}");

        // Simulate database lookup
        tokio::time::sleep(std::time::Duration::from_millis(10))
            .instrument(logfire::span!("Database query for user"))
            .await;

        logfire::debug!("Database query completed for user {user_id}",);

        if user_id == 0 {
            logfire::warn!("Invalid user ID requested: {user_id}",);
            return Err(StatusCode::BAD_REQUEST);
        }

        if user_id > 1000 {
            logfire::error!("User {user_id} not found");
            return Err(StatusCode::NOT_FOUND);
        }

        let user = User {
            id: user_id,
            name: format!("User {user_id}"),
            email: format!("user{user_id}@example.com"),
        };

        logfire::info!("Successfully retrieved user {user_id}",);

        Ok(Json(user))
    }
    .instrument(logfire::span!("Fetching user {user_id}"))
    .await
}

async fn create_user(
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<User>), StatusCode> {
    async {
        logfire::info!(
            "Creating new user: {name} <{email}>",
            name = &payload.name,
            email = &payload.email
        );

        // Validate input
        if payload.name.is_empty() || payload.email.is_empty() {
            logfire::warn!("Invalid user data provided");
            return Err(StatusCode::BAD_REQUEST);
        }

        // Simulate user creation
        tokio::time::sleep(std::time::Duration::from_millis(20))
            .instrument(logfire::span!("Database user creation"))
            .await;

        let user = User {
            id: 42, // In a real app, this would be generated
            name: payload.name.clone(),
            email: payload.email.clone(),
        };

        logfire::info!(
            "Successfully created user {id} with name {name}",
            id = user.id,
            name = &user.name
        );

        Ok((StatusCode::CREATED, Json(user)))
    }
    .instrument(logfire::span!(
        "Creating user {name}",
        name = &payload.name,
        email = &payload.email
    ))
    .await
}

async fn health_check(headers: HeaderMap) -> impl IntoResponse {
    async {
        let user_agent = headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        logfire::debug!(
            "Health check from user-agent: {user_agent}",
            user_agent = user_agent
        );

        Json(serde_json::json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "version": env!("CARGO_PKG_VERSION")
        }))
    }
    .instrument(logfire::span!("Health check"))
    .await
}

/// Custom middleware to demonstrate recording a custom metric via logfire
async fn metrics_middleware(request: axum::extract::Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();

    let response = next.run(request).await;

    // Increment request counter with labels
    REQUEST_COUNTER.add(
        1,
        &[
            KeyValue::new("method", method.to_string()),
            KeyValue::new("status_code", response.status().as_u16() as i64),
            KeyValue::new("route", uri.path().to_string()),
        ],
    );

    response
}
