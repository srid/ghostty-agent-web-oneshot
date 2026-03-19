{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        client = pkgs.buildNpmPackage {
          pname = "ghostty-agent-web-client";
          version = "0.1.0";
          src = ./client;
          npmDepsHash = "sha256-CWPB0PeLtL+oCmXzD/iGsDH+aFeTmIPItYXPTAX2BpU=";
          NODE_OPTIONS = "--max-old-space-size=4096";
          installPhase = ''
            runHook preInstall
            mkdir -p $out
            cp -r dist/* $out/
            runHook postInstall
          '';
        };

        server = pkgs.buildNpmPackage {
          pname = "ghostty-agent-web-server";
          version = "0.1.0";
          src = ./server;
          npmDepsHash = "sha256-8FeKDfAq/lqt+KS7LXySmpQ8B+x9pOzrd+qCv6V1KgI=";
          makeCacheWritable = true;
          nativeBuildInputs = with pkgs; [ python3 ];
          dontNpmBuild = true;
          postInstall = ''
            # Fix node-pty spawn-helper permissions
            find $out -name spawn-helper -exec chmod +x {} \;
            # Link client dist into server
            ln -s ${client} $out/lib/node_modules/ghostty-agent-web-server/client-dist
          '';
        };
      in
      {
        packages.default = pkgs.writeShellApplication {
          name = "ghostty-agent-web";
          runtimeInputs = [ pkgs.nodejs ];
          text = ''
            export GHOSTTY_AGENT_WEB_CLIENT_DIST="${client}"
            exec ${pkgs.nodejs}/bin/node ${server}/lib/node_modules/ghostty-agent-web-server/src/index.js "$@"
          '';
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [ nodejs python3 just ];
        };
      }
    );
}
