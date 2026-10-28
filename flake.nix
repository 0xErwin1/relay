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
            xorg.libX11
            xorg.libxcb
            xorg.libXcursor
            xorg.libXrandr
            xorg.libXi
            vulkan-loader
            alsa-lib
          ];

        nativeBuildInputs =
          with pkgs;
          [
            pkg-config
            cmake
          ]
          ++ lib.optionals stdenv.hostPlatform.isLinux [ mold ];

        # GPUI loads these with dlopen at runtime.
        runtimeLibraryPath = lib.makeLibraryPath (
          with pkgs;
          lib.optionals stdenv.hostPlatform.isLinux [
            wayland
            libxkbcommon
            vulkan-loader
            xorg.libX11
            xorg.libxcb
            fontconfig
          ]
        );
      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs;
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
