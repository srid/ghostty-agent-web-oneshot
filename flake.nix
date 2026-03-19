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

      perSystem = { self', pkgs, lib, ... }: {
        rust-project = {
          crateNixFile = "crate.nix";
        };

        # Build the client WASM bundle with trunk
        packages.client = pkgs.stdenv.mkDerivation {
          pname = "ghostty-agent-web-client";
          version = "0.1.0";
          src = ./.;
          nativeBuildInputs = with pkgs; [
            trunk
            wasm-bindgen-cli
            nodejs # needed for ghostty-web npm package
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.CoreServices
          ];
          buildPhase = ''
            cd client
            trunk build --release
          '';
          installPhase = ''
            mkdir -p $out
            cp -r dist/* $out/
          '';
        };

        # Combined package: server binary + client dist
        packages.default = pkgs.writeShellApplication {
          name = "ghostty-agent-web";
          runtimeInputs = [ ];
          text = ''
            export GHOSTTY_AGENT_WEB_CLIENT_DIST="${self'.packages.client}"
            exec ${self'.packages.ghostty-agent-web-server}/bin/ghostty-agent-web-server "$@"
          '';
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ self'.devShells.rust ];
          packages = with pkgs; [
            trunk
            wasm-bindgen-cli
            just
            nodejs # for ghostty-web npm package in trunk build
          ];
        };
      };
    };
}
