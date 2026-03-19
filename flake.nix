{
  description = "ghostty-agent-web — web terminal dashboard for coding agent sessions";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-flake.url = "github:juspay/rust-flake";
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      imports = [
        inputs.rust-flake.flakeModules.default
        inputs.rust-flake.flakeModules.nixpkgs
      ];

      perSystem = { self', pkgs, config, lib, ... }:
        let
          craneLib = config.rust-project.crane-lib;
          src = lib.cleanSource ./.;

          # Pre-fetch ghostty-web npm package
          ghosttyWebTgz = pkgs.fetchurl {
            url = "https://registry.npmjs.org/ghostty-web/-/ghostty-web-0.3.0.tgz";
            hash = "sha256-QFp6hW9OA5Nxu2w4CbhwP3Tqu48vDBURXkYrmWFpZWA=";
          };

          ghosttyWeb = pkgs.stdenv.mkDerivation {
            pname = "ghostty-web";
            version = "0.3.0";
            src = ghosttyWebTgz;
            phases = [ "unpackPhase" "installPhase" ];
            unpackPhase = ''
              tar xzf $src
            '';
            installPhase = ''
              cp -r package $out
            '';
          };

          # Build the client WASM binary with crane
          clientWasm = craneLib.buildPackage {
            pname = "ghostty-agent-web-client";
            version = "0.1.0";
            inherit src;
            cargoExtraArgs = "-p ghostty-agent-web-client --target wasm32-unknown-unknown";
            doCheck = false;
            CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          };

          # Assemble client dist: wasm-bindgen output + ghostty-web + HTML/CSS/JS
          clientDist = pkgs.stdenv.mkDerivation {
            pname = "ghostty-agent-web-client-dist";
            version = "0.1.0";
            src = ./client;
            nativeBuildInputs = [ pkgs.wasm-bindgen-cli pkgs.binaryen ];
            buildPhase = ''
              mkdir -p dist

              # Run wasm-bindgen
              wasm-bindgen \
                ${clientWasm}/bin/ghostty-agent-web-client.wasm \
                --out-dir dist \
                --target web \
                --no-typescript

              # Optimize WASM
              wasm-opt -Os dist/ghostty-agent-web-client_bg.wasm -o dist/ghostty-agent-web-client_bg.wasm || true

              # Copy ghostty-web library files
              cp ${ghosttyWeb}/dist/ghostty-web.js dist/
              cp ${ghosttyWeb}/ghostty-vt.wasm dist/ 2>/dev/null || \
                cp ${ghosttyWeb}/dist/ghostty-vt.wasm dist/ 2>/dev/null || true

              # Copy our JS bridge
              cp js/ghostty-bridge.js dist/

              # Copy static assets
              cp style.css dist/

              # Generate index.html that loads the WASM app
              cat > dist/index.html <<'INDEXEOF'
              <!DOCTYPE html>
              <html lang="en">
              <head>
                <meta charset="UTF-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1.0" />
                <title>ghostty-agent-web</title>
                <link rel="stylesheet" href="style.css" />
              </head>
              <body>
                <script type="module">
                  import init from './ghostty-agent-web-client.js';
                  await init();
                </script>
              </body>
              </html>
INDEXEOF
            '';
            installPhase = ''
              mkdir -p $out
              cp -r dist/* $out/
            '';
          };
        in
        {
          rust-project = {
            crateNixFile = "crate.nix";
          };

          packages.client = clientDist;

          # Combined package: server binary + client dist
          packages.default = pkgs.writeShellApplication {
            name = "ghostty-agent-web";
            text = ''
              export GHOSTTY_AGENT_WEB_CLIENT_DIST="${clientDist}"
              exec ${self'.packages.ghostty-agent-web-server}/bin/ghostty-agent-web-server "$@"
            '';
          };

          devShells.default = pkgs.mkShell {
            inputsFrom = [ self'.devShells.rust ];
            packages = with pkgs; [
              trunk
              wasm-bindgen-cli
              just
              nodejs
              cargo-watch
            ];
          };
        };
    };
}
