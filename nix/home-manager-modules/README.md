# Home-Manager Modules

This directory contains home-manager modules for deploying your Rust binaries.

## Module Structure

Each module is a function that receives `{ flake }` and returns a home-manager module:

```nix
{ flake }:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  # Access packages from your flake for the current system
  myPkgs = flake.packages.${pkgs.stdenv.hostPlatform.system};
  cfg = config.programs.myproject.my-tool;  # or config.services.myproject.my-service
in
{
  # Define options here (see "Common Patterns" below)
  options = ...;

  # Apply configuration when enabled
  config = lib.mkIf cfg.enable {
    # Implementation depends on pattern (see "Common Patterns" below)
  };
}
```

## Option Namespaces

Choose the appropriate namespace based on your use case:

| Namespace                   | Use Case                                        |
| --------------------------- | ----------------------------------------------- |
| `programs.myproject.<name>` | CLI tools, applications that users run directly |
| `services.myproject.<name>` | Background services, daemons                    |

## Common Patterns

Choose the pattern based on your needs:

| Pattern              | When to Use                                               |
| -------------------- | --------------------------------------------------------- |
| Simple Package       | No configuration needed in Nix, just install the binary   |
| Declarative Options  | Want to configure the program through Nix options         |
| Systemd User Service | Need to run as a background daemon                        |

### Simple Package Installation

Just install the binary without any Nix configuration:

```nix
{
  options.programs.myproject.my-tool = {
    enable = lib.mkEnableOption "my tool";

    package = lib.mkOption {
      type = lib.types.package;
      default = myPkgs.my-tool;
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];
  };
}
```

### Declarative Options

Configure through Nix options and generate config files:

```nix
let
  jsonFormat = pkgs.formats.json { };
in
{
  options.programs.myproject.my-tool = {
    enable = lib.mkEnableOption "my tool";

    package = lib.mkOption {
      type = lib.types.package;
      default = myPkgs.my-tool;
    };

    settings = {
      logLevel = lib.mkOption {
        type = lib.types.enum [ "debug" "info" "warn" "error" ];
        default = "info";
      };
    };

    configFile = lib.mkOption {
      type = lib.types.path;
      internal = true;
      default = jsonFormat.generate "config.json" cfg.settings;
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];
    xdg.configFile."my-tool/config.json".source = cfg.configFile;
  };
}
```

### Systemd User Service

Run as a background daemon:

```nix
{
  options.services.myproject.my-service = {
    enable = lib.mkEnableOption "my service";

    package = lib.mkOption {
      type = lib.types.package;
      default = myPkgs.my-service;
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.my-service = {
      Unit = {
        Description = "My Service";
        After = [ "network.target" ];
      };

      Service = {
        ExecStart = "${cfg.package}/bin/my-service";
        Restart = "on-failure";
      };

      Install = {
        WantedBy = [ "default.target" ];
      };
    };
  };
}
```

## Registering a Module

Add your module to `default.nix`:

```nix
imports = [
  (import ./my-module.nix { inherit flake; })
];
```

## Using in Home-Manager

```nix
{ inputs, ... }:

{
  imports = [
    inputs.my-project.homeManagerModules.default
  ];

  # Use whichever you defined in your module
  programs.myproject.my-tool.enable = true;
  # services.myproject.my-service.enable = true;
}
```

## Key Points

- **`{ flake }` parameter**: Receives your project's flake, access packages via `flake.packages.${system}`
- **`lib.mkIf cfg.enable`**: Only apply config when enabled
- **XDG paths**: For user data, prefer `${config.xdg.dataHome}/myproject/`
