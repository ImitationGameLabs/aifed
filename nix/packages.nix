{
  pkgs,
  lib,
  common,
}:
let
  inherit (common)
    craneLib
    cargoArtifacts
    src
    gitVersion
    ;

  aifed = craneLib.buildPackage {
    inherit cargoArtifacts src;
    version = gitVersion;
    strictDeps = true;

    nativeBuildInputs = with pkgs; [ pkg-config ];

    buildInputs =
      with pkgs;
      [
        openssl
      ]
      ++ lib.optionals pkgs.stdenv.isDarwin [
        pkgs.libiconv
      ];

    # Tests run via the `aifed-nextest` check (nix/checks.nix), not in this
    # package build. Keep `doCheck = false` scoped here; do NOT hoist it into
    # the shared commonArgs. crane's cargoNextest runs in checkPhase
    # (`doCheck = args.doCheck or true`), so a shared false would silently
    # disable aifed-nextest, and buildDepsOnly needs its default doCheck to
    # cache dev-deps via `cargo test --no-run`.
    doCheck = false;
  };
in
{
  inherit aifed;
  default = aifed;
}
