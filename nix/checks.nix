{
  pkgs,
  common,
  advisory-db,
  # Individual crate derivations
  myPkgs,
}:
let
  inherit (common)
    craneLib
    src
    commonArgs
    cargoArtifacts
    ;

  project = "aifed";
in
# Include all binary crates in checks (excludes meta packages like 'default')
(pkgs.lib.removeAttrs myPkgs [ "default" ])
// {
  # Run clippy (and deny all warnings) on the workspace source
  "${project}-clippy" = craneLib.cargoClippy (
    commonArgs
    // {
      inherit cargoArtifacts;
      cargoClippyExtraArgs = "--all-targets -- --deny warnings";
    }
  );

  # Build docs
  "${project}-doc" = craneLib.cargoDoc (
    commonArgs
    // {
      inherit cargoArtifacts;
      env.RUSTDOCFLAGS = "--deny warnings";
    }
  );

  # Check formatting
  "${project}-fmt" = craneLib.cargoFmt {
    inherit src;
  };

  # TOML formatting
  "${project}-toml-fmt" = craneLib.taploFmt {
    src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
  };

  # Audit dependencies for security issues
  "${project}-audit" = craneLib.cargoAudit {
    inherit src advisory-db;
  };

  # Audit licenses
  "${project}-deny" = craneLib.cargoDeny {
    inherit src;
  };

  # Run tests with cargo-nextest
  "${project}-nextest" = craneLib.cargoNextest (
    commonArgs
    // {
      inherit cargoArtifacts;
      partitions = 1;
      partitionType = "count";
      cargoNextestPartitionsExtraArgs = "--no-tests=pass";
    }
  );

  # Ensure that cargo-hakari is up to date (if you use workspace-hack)
  # my-project-hakari = craneLib.mkCargoDerivation {
  #   inherit src;
  #   pname = "my-project-hakari";
  #   cargoArtifacts = null;
  #   doInstallCargoArtifacts = false;
  #
  #   buildPhaseCargoCommand = ''
  #     cargo hakari generate --diff
  #     cargo hakari manage-deps --dry-run
  #     cargo hakari verify
  #   '';
  #
  #   nativeBuildInputs = [ pkgs.cargo-hakari ];
  # };
}
