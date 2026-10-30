{
  description = "Relay - a local-first HTTP client";

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

        relayVersion = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;

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

        # Build the `relay` package with rustPlatform, like DBFlux's non-flake
        # path in default.nix. Builds `-p relay` so the automation crates and
        # their capture libraries stay out of the package.
        #
        # Only tracked files reach the build in a git checkout, but filter the
        # common build output anyway so a plain directory copy also builds.
        relaySrc = lib.cleanSourceWith {
          src = ./.;
          filter =
            path: _type:
            (builtins.match ".*\\.git" path) == null
            && !lib.hasInfix "/target/" path
            # Root `nix build` symlinks: the final segment must be exactly
            # `result` or `result-*`, so `results.rs` and a `results/` module
            # survive.
            && (builtins.match ".*(/result)" path) == null
            && (builtins.match ".*(/result-[^/]*)" path) == null;
        };

        relayDesktop = ./assets/icon/relay.desktop;

        relay = pkgs.rustPlatform.buildRustPackage {
          pname = "relay";
          version = relayVersion;
          src = relaySrc;

          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };

          inherit buildInputs;
          nativeBuildInputs = nativeBuildInputs ++ [ pkgs.makeWrapper ];

          cargoBuildFlags = [
            "-p"
            "relay"
          ];

          # Tests run in CI (.github/workflows/ci.yml), not here: GPUI tests
          # need a display, which the build sandbox does not have.
          doCheck = false;

          postInstall = ''
            install -Dm644 ${relayDesktop} $out/share/applications/relay.desktop
            install -Dm644 ${./assets/icon/relay.svg} $out/share/icons/hicolor/scalable/apps/relay.svg
            install -Dm644 ${./assets/icon/relay-16.png} $out/share/icons/hicolor/16x16/apps/relay.png
            install -Dm644 ${./assets/icon/relay-32.png} $out/share/icons/hicolor/32x32/apps/relay.png
            install -Dm644 ${./assets/icon/relay-48.png} $out/share/icons/hicolor/48x48/apps/relay.png
            install -Dm644 ${./assets/icon/relay-64.png} $out/share/icons/hicolor/64x64/apps/relay.png
            install -Dm644 ${./assets/icon/relay-128.png} $out/share/icons/hicolor/128x128/apps/relay.png
            install -Dm644 ${./assets/icon/relay-256.png} $out/share/icons/hicolor/256x256/apps/relay.png
          '';

          postFixup = lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
            wrapProgram $out/bin/relay \
              --prefix LD_LIBRARY_PATH : "${runtimeLibraryPath}" \
              --suffix LD_LIBRARY_PATH : "/run/opengl-driver/lib:/usr/lib/x86_64-linux-gnu:/usr/lib64:/usr/lib"
          '';

          meta = with lib; {
            description = "A local-first HTTP client";
            homepage = "https://github.com/0xErwin1/relay";
            license = licenses.asl20;
            platforms = platforms.linux;
            mainProgram = "relay";
          };
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = buildInputs ++ automationBuildInputs;
          nativeBuildInputs = nativeBuildInputs ++ [
            rustToolchain
            pkgs.cargo-nextest
            pkgs.cargo-deny
            # Rasterizes assets/icon/*.svg to the committed PNGs; see
            # ARCHITECTURE.md, "Packaging and releases".
            pkgs.imagemagick
            pkgs.python3
          ];
          LD_LIBRARY_PATH = runtimeLibraryPath;
        };

        packages = {
          default = relay;
          relay = relay;
        };

        apps = {
          default = flake-utils.lib.mkApp {
            drv = relay;
            exePath = "/bin/relay";
          };
          relay = flake-utils.lib.mkApp {
            drv = relay;
            exePath = "/bin/relay";
          };
        };

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
