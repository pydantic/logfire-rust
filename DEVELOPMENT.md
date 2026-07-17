# Release Process

All crates (`logfire`, `logfire-core`, `logfire-client`) are versioned in
lockstep and published together from a single GitHub Release tagged
`v{version}`.

To release:

1. In a PR: bump `version` in every `{crate}/Cargo.toml` and the `logfire-core`
   pin in `[workspace.dependencies]` in the root `Cargo.toml` (all to the same
   version), update `logfire/CHANGELOG.md`, and merge to `main`.
2. Publish a GitHub Release with a new `v{version}` tag on `main`, e.g.
   `gh release create v0.12.0 --generate-notes`.
3. CI (`.github/workflows/main.yml`) runs tests on the tag, then the `release`
   job pauses in the `release` environment until a maintainer approves the
   deployment. On approval it runs `./release.sh`, which checks that every
   crate's version matches the tag and publishes with
   `cargo publish --workspace` (`logfire-core` first — cargo orders
   intra-workspace dependencies and waits for the index).

Publishing authenticates via crates.io
[trusted publishing](https://crates.io/docs/trusted-publishing): the job mints
a short-lived OIDC token with `rust-lang/crates-io-auth-action` — there is no
long-lived registry token. Each crate's trusted-publisher configuration on
crates.io must name repository `pydantic/logfire-rust`, workflow `main.yml`,
and environment `release`.
