#!/usr/bin/env bash
#
# Cut a release by pushing crate-prefixed git tags in dependency order.
#
# Actual publishing is done by CI (.github/workflows/main.yml): pushing a tag
# matching `{crate}-v*` runs `cargo publish -p {crate}` once tests/lint pass.
# This script only creates and pushes those tags.
#
# Why ordering matters: `logfire` and `logfire-client` both depend on
# `logfire-core` via a version requirement, so `logfire-core` must be live on
# crates.io *before* their tags are pushed. This script tags `logfire-core`
# first, waits for it to appear on crates.io, then tags the dependents.
#
# Versions are read straight from each crate's Cargo.toml — bump them (and the
# CHANGELOG) in a separate PR and merge to main before running this.
#
# Usage:
#   ./release.sh            # show the plan, then prompt before pushing tags
#   ./release.sh --yes      # skip the confirmation prompt
#   ./release.sh --dry-run  # show the plan and exit without tagging

set -euo pipefail

ASSUME_YES=false
DRY_RUN=false
for arg in "$@"; do
  case "$arg" in
    --yes|-y) ASSUME_YES=true ;;
    --dry-run|-n) DRY_RUN=true ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

# Crates in publish order. logfire-core has no intra-workspace deps and must be
# first; logfire and logfire-client depend on it.
CRATES=(logfire-core logfire logfire-client)

crate_version() {
  # First `version = "..."` line in the crate's Cargo.toml.
  grep -m1 '^version = ' "$1/Cargo.toml" | sed -E 's/^version = "(.*)"/\1/'
}

is_published() {
  # Succeeds if the exact {crate} {version} already exists on crates.io.
  curl -fsS "https://crates.io/api/v1/crates/$1/$2" >/dev/null 2>&1
}

wait_for_publish() {
  local crate="$1" version="$2"
  echo "Waiting for $crate $version to appear on crates.io (CI is publishing)..."
  for _ in $(seq 1 90); do
    if is_published "$crate" "$version"; then
      echo "  $crate $version is live."
      return 0
    fi
    sleep 10
  done
  echo "  timed out after 15m waiting for $crate $version" >&2
  return 1
}

# --- pre-flight checks -------------------------------------------------------

branch=$(git rev-parse --abbrev-ref HEAD)
if [ "$branch" != "main" ]; then
  echo "warning: you are on '$branch', not 'main'. Releases are normally cut from main." >&2
fi

if [ -n "$(git status --porcelain)" ]; then
  echo "error: working tree is not clean; commit or stash first." >&2
  exit 1
fi

echo "Fetching tags from origin..."
git fetch --quiet --tags origin

# --- build the plan ----------------------------------------------------------

declare -a to_tag_crate to_tag_version
echo
echo "Release plan:"
for crate in "${CRATES[@]}"; do
  version=$(crate_version "$crate")
  tag="$crate-v$version"
  if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
    echo "  $crate $version — tag $tag already exists locally, skip"
  elif is_published "$crate" "$version"; then
    echo "  $crate $version — already on crates.io, skip"
  else
    echo "  $crate $version — will tag and push '$tag'"
    to_tag_crate+=("$crate")
    to_tag_version+=("$version")
  fi
done

if [ "${#to_tag_crate[@]}" -eq 0 ]; then
  echo
  echo "Nothing to release. All crate versions are already tagged or published."
  exit 0
fi

if [ "$DRY_RUN" = true ]; then
  echo
  echo "(dry run — no tags pushed)"
  exit 0
fi

if [ "$ASSUME_YES" != true ]; then
  echo
  read -r -p "Push these tags? [y/N] " reply
  case "$reply" in
    y|Y|yes|Yes) ;;
    *) echo "aborted."; exit 1 ;;
  esac
fi

# --- push tags in dependency order -------------------------------------------

for i in "${!to_tag_crate[@]}"; do
  crate="${to_tag_crate[$i]}"
  version="${to_tag_version[$i]}"
  tag="$crate-v$version"

  echo
  echo "Tagging $tag..."
  git tag -a "$tag" -m "$crate $version"
  git push origin "$tag"
  echo "Pushed $tag — CI will publish $crate $version."

  # logfire-core must be live before the crates that depend on it are tagged.
  if [ "$crate" = "logfire-core" ]; then
    wait_for_publish logfire-core "$version"
  fi
done

echo
echo "Done. Watch CI for publish status: https://github.com/pydantic/logfire-rust/actions"
