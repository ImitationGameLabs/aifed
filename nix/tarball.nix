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
  # Standard FHS dynamic-loader path on x86_64 Linux.
  fhs-interp = "/lib64/ld-linux-x86-64.so.2";
in
# x86_64-linux only: the patchelf'd interpreter is architecture-specific,
# so don't ship a broken artifact on other platforms.
lib.optionalAttrs (pkgs.stdenv.hostPlatform.isx86_64 && pkgs.stdenv.hostPlatform.isLinux) {
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

    tar -czf $out/aifed-${gitVersion}-linux-x86_64.tar.gz bin/
  '';
}
