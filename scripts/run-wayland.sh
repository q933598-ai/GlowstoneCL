#!/usr/bin/env sh
set -eu
exec "$(dirname "$0")/../target/release/glowstonecl" --backend wayland "$@"
