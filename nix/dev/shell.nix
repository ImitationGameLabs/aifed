{
  pkgs,
  common,
  checks,
}:
let
  inherit (common) craneLib;
in
craneLib.devShell {
  inherit checks;

  # Extra inputs can be added here; cargo and rustc are provided by default.
  packages = with pkgs; [
    # Rust
    cargo-hakari
    rust-analyzer

    # Nix
    nixd
    nixfmt
    statix

    # TOML toolkit (linter, formatter)
    taplo
  ];
}
