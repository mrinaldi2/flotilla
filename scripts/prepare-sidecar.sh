#!/bin/bash
set -euo pipefail

# Builds flotillad and stages it where Tauri expects external binaries
# (apps/desktop/src-tauri/binaries/flotillad-<target-triple>), so the
# desktop bundle ships the daemon as a signed sidecar. Run before
# `pnpm tauri build` or `pnpm tauri dev`.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
EXT=""
case "$TRIPLE" in
  *-windows-*) EXT=".exe" ;;
esac

# Build settings are FLOTILLA_*; the HERMES_* names from before the rename
# still count when the new one is unset. Only set values are carried over:
# an empty FLOTILLA_TEAM_ID would fail the build's Team ID check.
for name in TEAM_ID DEV_BUILD RELEASE_BUILD; do
  new="FLOTILLA_$name"
  old="HERMES_$name"
  if [ -z "${!new:-}" ] && [ -n "${!old:-}" ]; then
    export "$new=${!old}"
  fi
done

# flotillad recognises the owner's Mac app by the Team ID compiled in (H-110).
# A release with the placeholder would never recognise the signed app, so
# anything but a real Team ID stops here unless this is an explicit dev
# build (FLOTILLA_DEV_BUILD=1, as scripts/dev.sh sets). Windows recognises the
# app by its Program Files folder and needs no Team ID (ARCH-R38).
case "$TRIPLE" in
  *-apple-darwin)
    if [ -z "${FLOTILLA_DEV_BUILD:-}" ] && ! [[ "${FLOTILLA_TEAM_ID:-}" =~ ^[A-Z0-9]{10}$ && "$FLOTILLA_TEAM_ID" != 0000000000 ]]; then
      echo "prepare-sidecar: FLOTILLA_TEAM_ID must be the owner's 10-character Apple Team ID for a release build (got '${FLOTILLA_TEAM_ID:-}'); set FLOTILLA_DEV_BUILD=1 for a dev build" >&2
      exit 1
    fi
    ;;
esac
# Where to build, and then copy from, in this one step (H-029, CE-013 G2).
# A release (no FLOTILLA_DEV_BUILD, or FLOTILLA_RELEASE_BUILD=1) never trusts
# $CARGO_TARGET_DIR: it builds into this checkout's own target, which no
# other bot may write, so nothing but this build can be shipped. A dev
# build uses the bot's own target when the session names one.
if [ -z "${FLOTILLA_DEV_BUILD:-}" ] || [ -n "${FLOTILLA_RELEASE_BUILD:-}" ]; then
  TARGET="$ROOT/target"
else
  TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
fi
cargo build --release -p flotillad --manifest-path "$ROOT/Cargo.toml" --target-dir "$TARGET"

# The gate asserts on the binary itself, not the build log (H-114): a macOS
# release must say it is signed with this Team ID.
IDENTITY="$("$TARGET/release/flotillad$EXT" --version | sed -n 's/^identity: //p')"
echo "identity: $IDENTITY"
case "$TRIPLE" in
  *-apple-darwin)
    if [ -z "${FLOTILLA_DEV_BUILD:-}" ] && [ "$IDENTITY" != "signed $FLOTILLA_TEAM_ID" ]; then
      echo "prepare-sidecar: the built flotillad reports '$IDENTITY', not 'signed $FLOTILLA_TEAM_ID'" >&2
      exit 1
    fi
    ;;
esac

DEST="$ROOT/apps/desktop/src-tauri/binaries"
mkdir -p "$DEST"
cp "$TARGET/release/flotillad$EXT" "$DEST/flotillad-$TRIPLE$EXT"
echo "staged $DEST/flotillad-$TRIPLE$EXT"
