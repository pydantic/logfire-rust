#[doc(hidden)]
#[path = "impl_.rs"]
pub mod __macros_impl;

/// Create a new [`Span`][tracing::Span]. This macro is a simplified version of `tracing::span!` which accepts
/// a smaller set of possible arguments and syntaxes.
///
/// In exchange, the macro automatically adds metadata which leads the span and its fields
/// to present nicely in the Pydantic Logfire UI.
///
/// # Syntax
///
/// The macro accepts the following syntax:
///
/// ```rust
/// # let value = 42;
/// logfire::span!(
///    parent: tracing::Span::current(), // optional, tracing::Span
///    level: tracing::Level::INFO,      // optional, tracing::Level
///    "format string",                  // required, must be a format string accepted by `format!()`
///    attr = value,                     // optional, attribute key and value pair
///    // ..                             // as many additional attr = value pairs as desired
/// );
/// ```
///
/// ## Attributes
///
/// The `attr = value` pairs are captured as [attributes] on the span.
///
/// `dotted.name = value` is also supported (and encouraged by Opentelemetry) to namespace
/// attributes. However, dotted names are not supported in Rust format strings.
///
/// ## Formatting
///
/// The format string only accepts arguments by name. These can either be explicitly passed
/// to `span!` as an attribute or captured from the surrounding scope by the macro. If captured
/// from the surrounding scope, they will only be used as a format string and not exported
/// as attributes.
///
/// # Examples
///
/// ```rust,no_run
/// // Span without any arguments
/// let root_span = logfire::span!("Root span");
///
/// // Span with attributes x and y
/// let span = logfire::span!(parent: &root_span, "Child span", x = 42, y = "hello");
///
/// // Typically a span will be "entered" to set the parent implicitly
/// root_span.in_scope(|| {
///     // This span will be a child of root_span
///     let child_span = logfire::span!("Nested span", x = 42, y = "hello");
///
///     // Debug-level child span
///     let debug_span = logfire::span!(level: tracing::Level::DEBUG, "Debugging", x = 42, y = "hello");
/// });
///
/// // With x included in the formatted message but not as an attribute
/// let x = 42;
/// let span = logfire::span!("Span with x = {x}, y = {y}", y = "hello");
///
/// // Attributes can either be a single name or a dotted.name
/// // `dotted.name` is not available in the format string.
/// let span = logfire::span!("Span with x = {x}, y = {y}", y = "hello", foo.bar = 42);
///
/// // If just an identifier is used without `= value`, it will be captured as an attribute
/// let foo = "bar";
/// let span = logfire::span!("Span with foo = {foo}", foo);
///
/// // This identifier shorthand also works for struct fields
/// #[derive(Debug)]
/// struct User {
///    email: &'static str,
///    name: &'static str,
/// }
///
/// let user = User {
///    email: "user@example.com",
///    name: "John Doe",
/// };
///
/// // NB the dotted name is not available in the format string
/// let span = logfire::span!("Span user info", user.email, user.name);
/// ```
///
/// [attributes]: https://opentelemetry.io/docs/concepts/signals/traces/#attributes
#[macro_export]
macro_rules! span {
    (parent: $parent:expr, level: $level:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::__macros_impl::tracing_span!(parent: $parent, $level, $format, $($($path).+ $(= $value)?),*)
    };
    (parent: $parent:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::__macros_impl::tracing_span!(parent: $parent, tracing::Level::INFO, $format, $($($path).+ $(= $value)?),*)
    };
    (level: $level:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::__macros_impl::tracing_span!($level, $format, $($($path).+ $(= $value)?),*)
    };
    ($format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::__macros_impl::tracing_span!(tracing::Level::INFO, $format, $($($path).+ $(= $value)?),*)
    };
}

/// Emit a log at the ERROR level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! error {
    (parent: $parent:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(parent: $parent, tracing::Level::ERROR, $format, $($($path).+ $(= $value)?),*)
    };
    ($format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(tracing::Level::ERROR, $format, $($($path).+ $(= $value)?),*)
    };
}

/// Emit a log at the WARN level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! warn {
    (parent: $parent:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(parent: $parent, tracing::Level::WARN, $format, $($($path).+ $(= $value)?),*)
    };
    ($format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(tracing::Level::WARN, $format, $($($path).+ $(= $value)?),*)
    };
}

/// Emit a log at the INFO level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! info {
    (parent: $parent:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(parent: $parent, tracing::Level::INFO, $format, $($($path).+ $(= $value)?),*)
    };
    ($format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(tracing::Level::INFO, $format, $($($path).+ $(= $value)?),*)
    };
}

/// Emit a log at the DEBUG level.
///
/// See the [`log!`][macro@crate::log] macro for more details.
#[macro_export]
macro_rules! debug {
    (parent: $parent:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(parent: $parent, tracing::Level::DEBUG, $format, $($($path).+ $(= $value)?),*)
    };
    ($format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::log!(tracing::Level::DEBUG, $format, $($($path).+ $(= $value)?),*)
    };
}

/// Export a log message at the specified level.
///
/// # Syntax
///
/// The macro accepts the following syntax:
///
/// ```rust
/// # let value = 42;
/// logfire::log!(
///    parent: tracing::Span::current(), // optional, tracing::Span
///    tracing::Level::INFO,             // required, see `info!` and variants for convenience
///    "format string",                  // required, must be a format string accepted by `format!()`
///    attr = value,                     // optional, attribute key and value pair
///    // ..                             // as many additional attr = value pairs as desired
/// );
/// ```
///
/// ## Attributes
///
/// The `attr = value` pairs are captured as [attributes] on the span.
///
/// `dotted.name = value` is also supported (and encouraged by Opentelemetry) to namespace
/// attributes. However, dotted names are not supported in Rust format strings.
//
/// ## Formatting
///
/// The format string only accepts arguments by name. These can either be explicitly passed
/// to `span!` as an attribute or captured from the surrounding scope by the macro. If captured
/// from the surrounding scope, they will only be used as a format string and not exported
/// as attributes.
///
/// # Examples
///
/// ```rust,no_run
/// use tracing::Level;
///
/// // Root span
/// let root_span = logfire::span!("Root span");
///
/// // Log with attributes x and y
/// logfire::log!(parent: &root_span, Level::INFO, "Child log", x = 42, y = "hello");
/// // or
/// logfire::info!(parent: &root_span, "Child log", x = 42, y = "hello");
///
/// // Typically a span will be "entered" to set the parent implicitly
/// root_span.in_scope(|| {
///     // This log will be a child of root_span
///     logfire::log!(Level::INFO, "Nested log", x = 42, y = "hello");
///     // or
///     logfire::info!("Nested log", x = 42, y = "hello");
///
///     // Debug-level child log
///     logfire::log!(Level::DEBUG, "Debugging", x = 42, y = "hello");
///     // or
///     logfire::debug!("Debugging", x = 42, y = "hello");
/// });
///
/// // With x included in the formatted message but not as an attribute
/// let x = 42;
/// logfire::log!(Level::INFO, "Log with x = {x}, y = {y}", y = "hello");
/// // or
/// logfire::info!("Log with x = {x}, y = {y}", y = "hello");
///
/// // Attributes can either be a single name or a dotted.name
/// // `dotted.name` is not available in the format string.
/// logfire::log!(Level::INFO, "Log with x = {x}, y = {y}", y = "hello", foo.bar = 42);
/// // or
/// logfire::info!("Log with x = {x}, y = {y}", y = "hello", foo.bar = 42);
///
/// // If just an identifier is used without `= value`, it will be captured as an attribute
/// let foo = "bar";
/// logfire::log!(Level::INFO, "Log with foo = {foo}", foo);
/// // or
/// logfire::info!("Log with foo = {foo}", foo);
///
/// // This identifier shorthand also works for struct fields
/// #[derive(Debug)]
/// struct User {
///    email: &'static str,
///    name: &'static str,
/// }
///
/// let user = User {
///    email: "user@example.com",
///    name: "John Doe",
/// };
///
/// // NB the dotted name is not available in the format string
/// logfire::log!(Level::INFO, "Log user info", user.email, user.name);
/// // or
/// logfire::info!("Log user info", user.email, user.name);
/// ```
///
/// [attributes]: https://opentelemetry.io/docs/concepts/signals/traces/#attributes
#[macro_export]
macro_rules! log {
    (parent: $parent:expr, $level:expr, $format:expr $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::__macros_impl::log!(parent: $parent, $level, $format, $($($path).+ $(= $value)?),*)
    };
    ($level:expr, $format:expr  $(, $($path:ident).+ $(= $value:expr)?)* $(,)?) => {
        $crate::__macros_impl::log!(parent: tracing::Span::current(), $level, $format, $($($path).+ $(= $value)?),*)
    };
}
