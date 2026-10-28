# Relay

A keyboard-first, local-first HTTP client built on [GPUI](https://www.gpui.rs/).
Relay reads and writes Postman collection v2.1 and environment files, runs Postman
`pm.*` scripts, and lets AI agents manage collections and send requests through MCP.

Workspaces are directories of small TOML files, so collections diff, merge and review
cleanly in git.

> Status: foundation. The workspace compiles and opens an empty window; no product
> features exist yet. See [ARCHITECTURE.md](ARCHITECTURE.md) for the design and roadmap.

## Quick path

```sh
nix develop          # toolchain, mold and native libraries
cargo run            # opens the Relay window
scripts/lint.py      # fmt, clippy, file-size and layering checks
cargo nextest run    # tests
```

Without Nix you need the Rust toolchain from `rust-toolchain.toml`, `mold`, `pkg-config`
and the GPUI native dependencies (Wayland, X11, xkbcommon, Vulkan, fontconfig, freetype).

## License

Dual-licensed under MIT or Apache-2.0, at your option.
