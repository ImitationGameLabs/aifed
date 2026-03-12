{
  description = "Nix Rust Template - A flake template for Rust projects with crane";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    inputs@{ self, flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      # Flake-level outputs
      flake = {
        # Export home-manager modules
        homeManagerModules.default = import ./nix/home-manager-modules { flake = self; };

        # Export templates for user initialization (optional)
        # templates.default = {
        #   path = ./templates/default;
        #   description = "Deployment configuration template";
        # };
      };

      perSystem =
        { system, lib, ... }:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import inputs.rust-overlay) ];
          };

          root = ./.;

          common = import ./nix/common.nix {
            inherit
              pkgs
              lib
              inputs
              root
              ;
          };

          packages = import ./nix/packages.nix {
            inherit pkgs lib common;
          };

          checks = import ./nix/dev/checks.nix {
            inherit pkgs common;
            inherit (inputs) advisory-db;
            myPkgs = packages;
          };
        in
        {
          inherit packages checks;

          devShells.default = import ./nix/dev/shell.nix {
            inherit pkgs common checks;
          };
        };
    };
}
