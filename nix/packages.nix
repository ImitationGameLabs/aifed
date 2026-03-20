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

    doCheck = false;
  };
in
{
  inherit aifed;
  default = aifed;
}
