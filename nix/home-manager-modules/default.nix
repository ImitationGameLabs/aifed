{ flake }:
{
  pkgs,
  lib,
  config,
  ...
}:
{
  imports = [
    (import ./aifed.nix { inherit flake; })
  ];
}
