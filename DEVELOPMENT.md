# Release Process

Each crate is versioned and released independently using prefixed git tags:

| Crate | Tag format | Example |
|-------|------------|---------|
| `logfire` | `logfire-v{version}` | `logfire-v0.9.0` |
| `logfire-core` | `logfire-core-v{version}` | `logfire-core-v0.1.0` |
| `logfire-client` | `logfire-client-v{version}` | `logfire-client-v0.1.0` |

To release a crate:

1. Update version in `{crate}/Cargo.toml`
2. Update `[workspace.dependencies]` in root `Cargo.toml` if releasing `logfire-core`
3. Commit: `git commit -am "Release {crate} v{version}"`
4. Tag: `git tag {crate}-v{version}`
5. Push: `git push && git push --tags`

CI will automatically publish to crates.io when the tag is pushed.

When releasing `logfire` or `logfire-client`, ensure `logfire-core` is published first if it has changes.
