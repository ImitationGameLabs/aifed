{
  pkgs,
  lib,
  inputs,
  # Project root path, must be passed from flake.nix (not ./.) because:
  # - Nix paths are evaluated at definition site
  # - If we use ./. here, it would resolve to nix/ directory, not project root
  # - Passing from flake.nix ensures ./. resolves to the correct location
  root,
}:
let
  # NB: we don't need to overlay our custom toolchain for the *entire*
  # pkgs (which would require rebuidling anything else which uses rust).
  # Instead, we just want to update the scope that crane will use by appending
  # our specific toolchain there.
  craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (
    p:
    p.rust-bin.stable.latest.default.override {
      extensions = [ "rust-src" ];
      # targets = [ "wasm32-wasip1" ];
    }
  );

  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      (lib.fileset.fromSource (craneLib.cleanCargoSource root))
      (root + /crates/aifed/src/skill.md)
    ];
  };

  # Use git shortRev as version, fallback to "dirty" if working tree is dirty
  gitVersion = inputs.self.shortRev or "dirty";

  # Common build arguments shared across all crate builds.
  # Includes source path, dependencies, and platform-specific inputs.
  commonArgs = {
    inherit src;
    strictDeps = true;

    nativeBuildInputs = with pkgs; [
      pkg-config
    ];

    buildInputs =
      with pkgs;
      [
        openssl
      ]
      ++ lib.optionals pkgs.stdenv.isDarwin [
        # Additional darwin specific inputs can be set here
        pkgs.libiconv
      ];
  };

  # Build *just* the cargo dependencies (of the entire workspace),
  # so we can reuse all of that work (e.g. via cachix) when running in CI
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
{
  inherit
    craneLib
    src
    commonArgs
    cargoArtifacts
    gitVersion
    ;
}
