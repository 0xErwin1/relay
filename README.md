# Relay

A local-first HTTP client built on [GPUI](https://www.gpui.rs/).
Relay reads and writes Postman collection v2.1 and environment files, runs Postman
`pm.*` scripts, and lets AI agents manage collections and send requests through MCP.

Workspaces are directories of small TOML files, so collections diff, merge and review
cleanly in git.

> Status: early. The window is still empty; `relay run` sends a request of a workspace
> from the command line. See [FEATURES.md](FEATURES.md) for what is planned and
> [ARCHITECTURE.md](ARCHITECTURE.md) for how it is built.

## Install

Linux only for now.

- **Nix**: from a clone of this repository, `nix build .#relay` builds the package
  (binary, desktop entry and icons) into `./result`; `nix profile install .#relay`
  installs it; `nix run .#relay` starts the app directly.
- **Nightly**: every push to `main` publishes a prerelease tagged `nightly-YYYYMMDD`
  with `relay-linux-amd64.tar.gz` (binary, desktop entry and icons) attached; see the
  [releases page](https://github.com/0xErwin1/relay/releases). Only the newest nightly
  is kept.
- **Local build**: from the dev shell (`nix develop`), `cargo build --release -p relay`
  puts the binary in `target/release/relay`.

## Quick path

```sh
nix develop          # toolchain, mold and native libraries
cargo run            # opens the Relay window (on the current directory if it holds relay.toml)
cargo run -- ~/billing                     # opens the workspace in ~/billing
cargo run -- run billing-api/invoices/list-invoices --workspace ~/billing --env staging
                     # sends one request, body to stdout, summary to stderr
scripts/lint.py      # fmt, clippy, file-size and layering checks
cargo nextest run    # tests
```

Without Nix you need the Rust toolchain from `rust-toolchain.toml`, `mold`, `pkg-config`
and the GPUI native dependencies (Wayland, X11, xkbcommon, Vulkan, fontconfig, freetype).

## License

Licensed under the [Apache License 2.0](LICENSE-APACHE).
