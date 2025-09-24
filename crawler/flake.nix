# This flake is built off this template: https://github.com/the-nix-way/dev-templates/blob/main/rust/flake.nix
{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.1";
    fenix = {
      url = "https://flakehub.com/f/nix-community/fenix/0.1";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs: let
    supportedSystems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];
    forEachSupportedSystem = f:
      inputs.nixpkgs.lib.genAttrs supportedSystems (system:
        f {
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [
              inputs.self.overlays.default
            ];
          };
        });
  in {
    overlays.default = final: prev: {
      rustToolchain = with inputs.fenix.packages.${prev.stdenv.hostPlatform.system};
        combine (with stable; [
          clippy
          rustc
          cargo
          rustfmt
          rust-src
        ]);
    };

    devShells = forEachSupportedSystem ({pkgs}: {
      default = pkgs.mkShell {
        packages = with pkgs; [
          # Rust packages
          rustToolchain
          pkg-config
          rust-analyzer

          # Required for reqwest crate
          openssl

          # Simple script to connect to postgresql database
          (writeShellScriptBin "connect" ''
            psql -h localhost -p 5432 -U search_db_user -d search_db
          '')
        ];

        env = {
          # Required by rust-analyzer
          RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";

          # Required by the program to connect to the db
          DATABASE_URL = "postgresql://postgres:123@localhost:5432/postgres";
        };
      };
      nativeBuildInputs = with pkgs; [pkg-config];
    });
  };
}
