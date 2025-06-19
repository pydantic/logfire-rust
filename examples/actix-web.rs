//! Example demonstrating Logfire integration with Actix Web framework.
//!
//! This example shows how to:
//! - Set up Logfire with Actix Web
//! - Instrument HTTP requests with tracing and metrics middleware (OpenTelemetry)
//! - Log custom events within route handlers
//! - Create spans for business logic
//! - Track metrics for request counts
//!
//! Run with: `cargo run --example actix-web`
//! Make sure to set a write token as an environment variable (`LOGFIRE_TOKEN`)
//! <https://logfire.pydantic.dev/docs/how-to-guides/create-write-tokens>/

use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Result as ActixResult,
    middleware::{DefaultHeaders, Logger},
    web,
};
use futures_util::future::{LocalBoxFuture, Ready, ready};
use opentelemetry::KeyValue;
use opentelemetry::metrics::Counter;
use opentelemetry_instrumentation_actix_web::{RequestMetrics, RequestTracing};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::task::{Context, Poll};
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

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Logfire
    let shutdown_handler = logfire::configure()
        .install_panic_handler()
        .finish()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    logfire::info!("Starting Actix Web server with Logfire integration");

    HttpServer::new(|| {
        App::new()
            .wrap(RequestTracing::new())
            .wrap(RequestMetrics::default())
            .wrap(CustomRequestCount)
            .wrap(Logger::default())
            .wrap(DefaultHeaders::new().add(("X-Version", env!("CARGO_PKG_VERSION"))))
            .route("/", web::get().to(root))
            .route("/users/{id}", web::get().to(get_user))
            .route("/users", web::post().to(create_user))
            .route("/health", web::get().to(health_check))
    })
    .bind("127.0.0.1:3000")?
    .run()
    .await?;

    shutdown_handler.shutdown()?;

    Ok(())
}

async fn root() -> HttpResponse {
    async {
        logfire::info!("Root endpoint accessed");
        HttpResponse::Ok().body("Hello, Actix Web with Logfire!")
    }
    .instrument(logfire::span!("Handling root request"))
    .await
}

async fn get_user(path: web::Path<u32>) -> ActixResult<HttpResponse> {
    let user_id = path.into_inner();
    async {
        logfire::info!("Fetching user with ID: {user_id}", user_id = user_id as i64);

        // Simulate database lookup
        tokio::time::sleep(std::time::Duration::from_millis(10))
            .instrument(logfire::span!("Database query for user"))
            .await;

        logfire::debug!(
            "Database query completed for user {user_id}",
            user_id = user_id as i64
        );

        if user_id == 0 {
            logfire::warn!(
                "Invalid user ID requested: {user_id}",
                user_id = user_id as i64
            );
            return Ok(HttpResponse::BadRequest().finish());
        }

        if user_id > 1000 {
            logfire::error!("User {user_id} not found", user_id = user_id as i64);
            return Ok(HttpResponse::NotFound().finish());
        }

        let user = User {
            id: user_id,
            name: format!("User {user_id}"),
            email: format!("user{user_id}@example.com"),
        };

        logfire::info!(
            "Successfully retrieved user {user_id}",
            user_id = user_id as i64
        );

        Ok(HttpResponse::Ok().json(user))
    }
    .instrument(logfire::span!("Fetching user {user_id}", user_id = user_id))
    .await
}

async fn create_user(payload: web::Json<CreateUserRequest>) -> ActixResult<HttpResponse> {
    async {
        logfire::info!(
            "Creating new user: {name} <{email}>",
            name = &payload.name,
            email = &payload.email
        );

        // Validate input
        if payload.name.is_empty() || payload.email.is_empty() {
            logfire::warn!("Invalid user data provided");
            return Ok(HttpResponse::BadRequest().finish());
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
            id = user.id as i64,
            name = &user.name
        );

        Ok(HttpResponse::Created().json(user))
    }
    .instrument(logfire::span!(
        "Creating user {name}",
        name = &payload.name,
        email = &payload.email
    ))
    .await
}

async fn health_check(req: HttpRequest) -> HttpResponse {
    async {
        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        logfire::debug!(
            "Health check from user-agent: {user_agent}",
            user_agent = user_agent
        );

        HttpResponse::Ok().json(serde_json::json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "version": env!("CARGO_PKG_VERSION")
        }))
    }
    .instrument(logfire::span!("Health check"))
    .await
}

struct CustomRequestCount;

impl<S, B> Transform<S, ServiceRequest> for CustomRequestCount
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = CustomRequestCountMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CustomRequestCountMiddleware { service }))
    }
}

struct CustomRequestCountMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for CustomRequestCountMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let method = req.method().clone();
        let path = req.path().to_string();
        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            REQUEST_COUNTER.add(
                1,
                &[
                    KeyValue::new("method", method.to_string()),
                    KeyValue::new("route", path),
                    KeyValue::new("status_code", res.status().as_u16() as i64),
                ],
            );
            Ok(res)
        })
    }
}
