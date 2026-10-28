{
  description = "Relay - a keyboard-first, local-first HTTP client";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        lib = pkgs.lib;

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # Native libraries GPUI needs to build and run.
        buildInputs =
          with pkgs;
          [
            openssl
            zstd
            zlib
            freetype
            fontconfig
          ]
          ++ lib.optionals stdenv.hostPlatform.isLinux [
            wayland
            libxkbcommon
            libx11
            libxcb
            libxcursor
            libxrandr
            libxi
            vulkan-loader
            alsa-lib
          ];

        # scripts/gpui-mcp.sh builds the UI automation server, whose capture
        # backend links PipeWire, libgbm and EGL and generates bindings with
        # bindgen. Relay itself does not need them.
        automationBuildInputs = lib.optionals pkgs.stdenv.hostPlatform.isLinux (
          with pkgs;
          [
            pipewire
            libgbm
            libGL
          ]
        );

        nativeBuildInputs =
          with pkgs;
          [
            pkg-config
            cmake
            rustPlatform.bindgenHook
          ]
          ++ lib.optionals stdenv.hostPlatform.isLinux [ mold ];

        # GPUI loads these with dlopen at runtime.
        runtimeLibraryPath = lib.makeLibraryPath (
          with pkgs;
          lib.optionals stdenv.hostPlatform.isLinux [
            wayland
            libxkbcommon
            vulkan-loader
            libx11
            libxcb
            fontconfig
          ]
        );
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = buildInputs ++ automationBuildInputs;
          nativeBuildInputs = nativeBuildInputs ++ [
            rustToolchain
            pkgs.cargo-nextest
            pkgs.cargo-deny
            pkgs.python3
          ];
          LD_LIBRARY_PATH = runtimeLibraryPath;
        };

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
