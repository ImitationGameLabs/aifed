{ flake }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.aifed;

  tomlFormat = pkgs.formats.toml { };

  # Filter null values: TOML does not support null, so optional fields
  # (displayName, initializationOptions) must be omitted when not set.
  lspEntries = lib.mapAttrsToList (_name: lsp:
    lib.filterAttrs (_: v: v != null) {
      inherit (lsp) language command args;
      file_extensions = lsp.fileExtensions;
      root_markers = lsp.rootMarkers;
      display_name = lsp.displayName;
      initialization_options = lsp.initializationOptions;
    }
  ) cfg.lspServers;

  configFile = tomlFormat.generate "aifed-config.toml" { lsp = lspEntries; };
in
{
  options.programs.aifed = {
    enable = lib.mkEnableOption "aifed";

    package = lib.mkOption {
      type = lib.types.package;
      default = flake.packages.${pkgs.stdenv.hostPlatform.system}.aifed;
      description = "The aifed package to use.";
    };

    lspServers = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule {
          options = {
            language = lib.mkOption {
              type = lib.types.str;
              description = "Language identifier used by aifed and LSP requests.";
            };

            command = lib.mkOption {
              type = lib.types.str;
              description = "Executable used to launch the language server.";
            };

            fileExtensions = lib.mkOption {
              type = lib.types.listOf lib.types.str;
              default = [ ];
              description = "File extensions mapped to this language.";
            };

            rootMarkers = lib.mkOption {
              type = lib.types.listOf lib.types.str;
              default = [ ];
              description = "Workspace-root files that trigger auto-start.";
            };

            args = lib.mkOption {
              type = lib.types.listOf lib.types.str;
              default = [ ];
              description = "Arguments passed to the server process.";
            };

            displayName = lib.mkOption {
              type = lib.types.nullOr lib.types.str;
              default = null;
              description = "Human-readable server name for logs and status.";
            };

            initializationOptions = lib.mkOption {
              type = lib.types.nullOr lib.types.anything;
              default = null;
              description = "Initialization options passed during LSP initialize.";
            };
          };
        }
      );
      default = {
        rust = {
          language = "rust";
          command = "rust-analyzer";
          fileExtensions = [ "rs" ];
          rootMarkers = [ "Cargo.toml" ];
          displayName = "rust-analyzer";
          initializationOptions = {
            checkOnSave.command = "clippy";
            cargo.allFeatures = true;
          };
        };
      };
      description = "LSP server configurations, keyed by language name.";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile."aifed/config.toml".source = configFile;
  };
}
