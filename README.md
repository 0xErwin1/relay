# Relay

A local-first HTTP client built on [GPUI](https://www.gpui.rs/).
Relay reads and writes Postman collection v2.1 and environment files, runs Postman
`pm.*` scripts, and lets AI agents manage collections and send requests through MCP.

Workspaces are directories of small TOML files, so collections diff, merge and review
cleanly in git.

> Status: early. The window is still empty; `relay run` sends a request of a workspace
> from the command line. See [FEATURES.md](FEATURES.md) for what is planned and
> [ARCHITECTURE.md](ARCHITECTURE.md) for how it is built.

## Quick path

```sh
nix develop          # toolchain, mold and native libraries
cargo run            # opens the Relay window
cargo run -- run billing-api/invoices/list-invoices --workspace ~/billing --env staging
                     # sends one request, body to stdout, summary to stderr
scripts/lint.py      # fmt, clippy, file-size and layering checks
cargo nextest run    # tests
```

Without Nix you need the Rust toolchain from `rust-toolchain.toml`, `mold`, `pkg-config`
and the GPUI native dependencies (Wayland, X11, xkbcommon, Vulkan, fontconfig, freetype).

## License

Licensed under the [Apache License 2.0](LICENSE-APACHE).
