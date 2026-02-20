#!/usr/bin/env bash
# Workspace-safe version bumper for hydra-shed.
# Usage: ./scripts/bump-version.sh [patch|minor|major]
#
# cargo-bump panics on workspaces, so this script edits Cargo.toml directly.

set -euo pipefail

CARGO_TOML="Cargo.toml"
LEVEL="${1:-patch}"

# Extract current version from the first `version = "..."` line
CURRENT=$(grep '^version' "$CARGO_TOML" | head -1 | sed 's/.*"\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

case "$LEVEL" in
  patch) PATCH=$((PATCH + 1)) ;;
  minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
  major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
  *)
    echo "Usage: $0 [patch|minor|major]" >&2
    exit 1
    ;;
esac

NEW="${MAJOR}.${MINOR}.${PATCH}"

# Replace the first occurrence of version = "..." in Cargo.toml
# Uses awk instead of sed to avoid BSD/GNU sed incompatibilities.
awk -v old="$CURRENT" -v new="$NEW" '
  !done && /^version = "/ { sub(old, new); done=1 }
  { print }
' "$CARGO_TOML" > "${CARGO_TOML}.tmp" && mv "${CARGO_TOML}.tmp" "$CARGO_TOML"

echo "${CURRENT} -> ${NEW}"
