# Home-manager modules for deploying services
#
# Import this module in your home-manager configuration:
#   imports = [ your-flake.homeManagerModules.default ];
#
# Then enable services as needed:
#   services.myproject.my-service = { enable = true; ... };

{ flake }:
{
  pkgs,
  lib,
  config,
  ...
}:
{
  imports = [
    # Add your service modules here, e.g.:
    # (import ./my-service.nix { inherit flake; })
  ];

  # Add shared options/config if needed
  # options.services.myproject._configDir = ...
}
