#!/bin/sh
# Build the release binary the way CI does: nightly, with std rebuilt so the
# panic strategy can be upgraded from abort to immediate-abort.
#
# Usage: scripts/build-min.sh [target-triple]
set -eu

if ! rustc +nightly --print sysroot >/dev/null 2>&1; then
  echo "nightly toolchain required: rustup toolchain install nightly" >&2
  exit 1
fi

target=${1:-$(rustc +nightly -vV | sed -n 's/^host: //p')}

rustup component add rust-src --toolchain nightly >/dev/null

RUSTFLAGS="-Z unstable-options -C panic=immediate-abort" \
  cargo +nightly build --release --locked \
    --target "$target" \
    -Z build-std=std,panic_abort

echo "built target/$target/release/zenity-rs"
