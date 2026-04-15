# Instrumenting AWS Lambda with Pydantic Logfire

When running on AWS Lambda, extra care must be taken to ensure all telemetry is successfully exported to Logfire.

The AWS Lambda runtime will freeze lambda processes as soon as the response is delivered.
This means that background threads (such as those exporting telemetry to Logfire) will be paused.
To ensure that telemetry is exported successfully, it's necessary to flush Logfire before completing the Lambda invocation.

The following example demonstrates using Logfire with the `lambda_runtime` crate to instrument a Lambda function.
A `tower::Layer` is used to ensure that Logfire is flushed at the end of every invocation.

```rust,ignore
use std::{
    future::Future,
    task::{Context, Poll},
};

use lambda_runtime::{service_fn, Error, LambdaEvent};
use logfire::{config::ConsoleOptions, Logfire};
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use tower::{Layer, Service};

/// A `tower::Layer` that will be used to introduce flushing to the lambda function.
pub struct LogfireFlushLayer {
    logfire: Logfire,
}

impl<S> Layer<S> for LogfireFlushLayer {
    type Service = LogfireFlushService<S>;

    fn layer(&self, service: S) -> Self::Service {
        LogfireFlushService {
            logfire: self.logfire.clone(),
            service,
        }
    }
}

/// A `tower::Service` which wraps an inner service to flush Logfire when the service
/// finishes executing.
pub struct LogfireFlushService<S> {
    logfire: Logfire,
    service: S,
}

impl<S, Request> Service<Request> for LogfireFlushService<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = LogfireFlushFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        LogfireFlushFuture {
            inner: Some(self.service.call(request)),
            logfire: self.logfire.clone(),
        }
    }
}

/// The future produced when calling the `LogfireFlushService`. The future is
/// responsible for driving the inner future and then flushing logfire when
/// the inner future completes.
#[pin_project]
pub struct LogfireFlushFuture<F> {
    #[pin]
    inner: Option<F>,
    logfire: Logfire,
}

impl<F, T, E> Future for LogfireFlushFuture<F>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<T, E>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let Some(inner) = this.inner.as_mut().as_pin_mut() else {
            panic!("`LogfireFlushFuture` polled after completion");
        };
        match inner.poll(cx) {
            Poll::Ready(result) => {
                // Drop the inner future so that any spans it holds are dropped before flushing
                this.inner.set(None);
                // Flush logfire before returning.
                // Note that this is a blocking function. In the context of the current lambda
                // invocation that should not be a problem.
                let _ = this.logfire.force_flush();
                Poll::Ready(result)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Example lambda request payload.
#[derive(Deserialize)]
pub(crate) struct IncomingMessage {
    command: String,
}

/// Example lambda response payload.
#[derive(Serialize)]
pub(crate) struct OutgoingMessage {
    req_id: String,
    msg: String,
}

/// Main body of the lambda function.
#[tracing::instrument(skip_all)]
pub(crate) async fn function_handler(
    event: LambdaEvent<IncomingMessage>,
) -> Result<OutgoingMessage, Error> {

    // Change this logic to be whatever your lambda function needs to do.

    Ok(OutgoingMessage {
        req_id: event.context.request_id,
        msg: format!("Command {}.", event.payload.command),
    })
}

/// Main function for the lambda process.
#[tokio::main]
async fn main() -> Result<(), Error> {
    // 1. Configure logfire on startup
    let logfire = logfire::configure()
        .with_console(Some(ConsoleOptions::default()))
        .finish()?;
    logfire::info!("Starting up");

    // 2. Lambda processes require special termination logic. Logfire's
    //    shutdown guard can be passed into `lambda_runtime`'s graceful
    //    shutdown handler to ensure that telemetry is flushed when
    //    idle lambda processes are shutdown.
    let shutdown_guard = logfire.clone().shutdown_guard();
    lambda_runtime::spawn_graceful_shutdown_handler(|| async move {
        logfire::info!("Shutting down");
        let _ = shutdown_guard.shutdown();
    })
    .await;

    // 3. Prepare the main `lambda_runtime::Runtime`
    lambda_runtime::Runtime::new(service_fn(function_handler))
        // 4. Add a `TracingLayer` before the logfire layer
        .layer(lambda_runtime::layers::TracingLayer::new())
        // 5. Add the flushing layer after; this way the spans created
        //    by the `TracingLayer` will be closed before logfire is flushed.
        .layer(LogfireFlushLayer { logfire })
        // 6. And finally, run the process.
        .run()
        .await
}

```
