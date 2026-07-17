#!/usr/bin/env bash
#
# Publish all workspace crates to crates.io. Runs in CI
# (.github/workflows/main.yml) when a GitHub Release is published: the release
# job names the `release` environment, so it pauses for reviewer approval,
# mints a short-lived crates.io token via OIDC trusted publishing, and then
# runs this script.
#
# All crates are versioned in lockstep. This verifies every crate's Cargo.toml
# version matches the release tag (`v{version}`), then publishes with
# `cargo publish --workspace`, which orders intra-workspace dependencies
# (`logfire-core` before `logfire` / `logfire-client`) and waits for each
# publish to land in the index before publishing its dependents.
#
# Usage:
#   ./release.sh            # verify + publish (CI; needs CARGO_REGISTRY_TOKEN)
#   ./release.sh --dry-run  # verify versions + `cargo publish` dry run

set -euo pipefail

DRY_RUN=false
for arg in "$@"; do
  case "$arg" in
    --dry-run|-n) DRY_RUN=true ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

CRATES=(logfire-core logfire logfire-client)

crate_version() {
  # First `version = "..."` line in the crate's Cargo.toml.
  grep -m1 '^version = ' "$1/Cargo.toml" | sed -E 's/^version = "(.*)"/\1/'
}

version=$(crate_version "${CRATES[0]}")
for crate in "${CRATES[@]}"; do
  v=$(crate_version "$crate")
  if [ "$v" != "$version" ]; then
    echo "error: $crate is $v but ${CRATES[0]} is $version — crates are versioned in lockstep." >&2
    exit 1
  fi
done

# In CI the workflow runs on the release's tag; refuse to publish code whose
# versions don't match the tag that triggered it.
if [ -n "${GITHUB_REF_NAME:-}" ] && [ "$GITHUB_REF_NAME" != "v$version" ]; then
  echo "error: release tag '$GITHUB_REF_NAME' does not match crate version '$version' (expected 'v$version')." >&2
  exit 1
fi

echo "Publishing ${CRATES[*]} at $version..."
if [ "$DRY_RUN" = true ]; then
  cargo publish --workspace --dry-run
else
  cargo publish --workspace
fi
