#!/usr/bin/env bash
# Builds and runs the gpui-mcp UI automation server at the revision Relay's
# bridge uses, so both sides speak the same protocol version.
#
#   scripts/gpui-mcp.sh install     build it into .tools/ (run once, in nix develop)
#   scripts/gpui-mcp.sh [args...]   run it, e.g. as an MCP server: --app-id reqflux
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
rev="$(grep -oP 'themixednuts/gpui-mcp", rev = "\K[0-9a-f]{40}' "$root/Cargo.toml" | head -n1)"
tools="$root/.tools/gpui-mcp-$rev"
binary="$tools/bin/gpui-mcp"

if [[ -z "$rev" ]]; then
    echo "gpui-mcp.sh: no gpui-mcp rev found in Cargo.toml" >&2
    exit 1
fi

if [[ "${1:-}" == "install" ]]; then
    cargo install --locked --git https://github.com/themixednuts/gpui-mcp --rev "$rev" \
        --root "$tools" gpui-mcp-server
    exit 0
fi

if [[ ! -x "$binary" ]]; then
    echo "gpui-mcp.sh: $binary is missing; run 'scripts/gpui-mcp.sh install' first" >&2
    exit 1
fi

exec "$binary" "$@"
