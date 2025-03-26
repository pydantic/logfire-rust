## [v0.4.0] (2025-03-26)

* remove debug print, and add clippy lint to prevent future regressions by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/32
* support `dotted.attribute` syntax in the macros by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/34
* add `with_console_options` config, deprecate `console_mode` by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/33
* rename `with_metrics_options` to `with_metrics` by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/35
* add support for inferring token from region by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/36

## [v0.3.1] (2025-03-25)

* fix double-print to console of tracing events outside spans by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/30
* revert to only emit pending spans on first enter by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/29

## [v0.3.0] (2025-03-25)

* support printing tracing events (to console) by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/21
* fix double-print to console when `LOGFIRE_SEND_TO_LOGFIRE=no` by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/23
* fix print to console to emit spans in the right order by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/24
* support configuring metrics by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/25
* print tracing event fields by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/26
* emit log entries for top-level tracing events by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/27

## [v0.2.0] (2025-03-19)

* Add `span_exporter()` helper to build a span exporter directly in [#20](https://github.com/pydantic/logfire-rust/pull/20)
* Fix `set_resource` not being called on span processors added with `with_additional_span_processor()` in [#20](https://github.com/pydantic/logfire-rust/pull/20)

## [v0.1.0] (2025-03-13)

Initial release.

[v0.1.0]: https://github.com/pydantic/logfire-rust/commits/v0.1.0
[v0.2.0]: https://github.com/pydantic/logfire-rust/compare/v0.1.0..v0.2.0
[v0.3.0]: https://github.com/pydantic/logfire-rust/compare/v0.2.0..v0.3.0
[v0.3.1]: https://github.com/pydantic/logfire-rust/compare/v0.3.0..v0.3.1
[v0.4.0]: https://github.com/pydantic/logfire-rust/compare/v0.3.1..v0.4.0
