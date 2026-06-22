# nix/tarball.nix - FHS-ready tarball of the aifed binaries for distribution
# outside Nix (benchmark containers, standalone install). Reuses the aifed
# derivation rather than rebuilding the workspace. The patchelf'd binaries run
# on any standard Linux (Ubuntu, Debian, ...) but NOT inside Nix (the
# /nix/store loader is gone), so this is a separate output from aifed.

{
  pkgs,
  lib,
  aifed,
  gitVersion,
}:

let
  # FHS dynamic-loader path per Linux arch: where the loader lives on a stock
  # distro (not the nix store path). darwin and any unlisted arch fall through
  # to null, so no tarball is produced there (darwin has no ELF/FHS loader).
  fhs-interp =
    {
      x86_64-linux = "/lib64/ld-linux-x86-64.so.2";
      aarch64-linux = "/lib/ld-linux-aarch64.so.1";
    }
    .${pkgs.stdenv.hostPlatform.system} or null;
in
# Produces a tarball only where fhs-interp is defined (Linux arches above);
# darwin and unlisted arches get no output.
lib.optionalAttrs (fhs-interp != null) {
  aifed-tarball = pkgs.runCommand "aifed-tarball" { nativeBuildInputs = [ pkgs.patchelf ]; } ''
    mkdir -p $out
    cp -r ${aifed}/bin bin
    chmod -R u+w bin

    # Rewrite the ELF interpreter to the FHS path and drop the Nix rpath so
    # the binaries run on any standard Linux. tar is provided by stdenv.
    for bin in bin/*; do
      patchelf --set-interpreter ${fhs-interp} "$bin"
      patchelf --remove-rpath "$bin"
    done

    tar -czf $out/aifed-${gitVersion}.tar.gz bin/
  '';
}
