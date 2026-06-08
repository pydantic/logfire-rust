# Release Process

Each crate is versioned and released independently using prefixed git tags:

| Crate | Tag format | Example |
|-------|------------|---------|
| `logfire` | `logfire-v{version}` | `logfire-v0.9.0` |
| `logfire-core` | `logfire-core-v{version}` | `logfire-core-v0.1.0` |
| `logfire-client` | `logfire-client-v{version}` | `logfire-client-v0.1.0` |

To release:

1. In a PR: bump the version in each `{crate}/Cargo.toml` being released (and
   `[workspace.dependencies]` in the root `Cargo.toml` if releasing
   `logfire-core`), update `logfire/CHANGELOG.md`, and merge to `main`.
2. From `main`, run `./release.sh`. It reads the versions from `Cargo.toml`,
   shows a plan, and pushes the `{crate}-v{version}` tags in dependency order:
   `logfire-core` first (waiting for it to land on crates.io), then `logfire`
   and `logfire-client`. Use `--dry-run` to preview or `--yes` to skip the
   prompt.

CI (`.github/workflows/main.yml`) publishes each crate to crates.io when its
tag is pushed. `logfire` and `logfire-client` depend on `logfire-core`, so it
must be published first — `release.sh` enforces this ordering.
