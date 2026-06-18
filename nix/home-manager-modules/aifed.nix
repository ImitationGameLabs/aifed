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

  # Filter null values: TOML has no null, so optional fields (displayName,
  # initializationOptions) must be omitted when unset.
  lspEntries = lib.mapAttrsToList (
    _name: lsp:
    lib.filterAttrs (_: v: v != null) {
      inherit (lsp) language command args;
      root_markers = lsp.rootMarkers;
      display_name = lsp.displayName;
      initialization_options = lsp.initializationOptions;
    }
  ) cfg.lspServers;

  # `[[language]]` extension overlays layer on the in-code grammar defaults:
  # effective = (grammar_defaults ∪ additional) − exclude. Empty extension lists
  # are omitted; empty by default since grammar languages resolve from code.
  languageEntries = lib.mapAttrsToList (
    _name: lang:
    {
      inherit (lang) language;
    }
    // lib.optionalAttrs (lang.additionalExtensions != [ ]) {
      additional_extensions = lang.additionalExtensions;
    }
    // lib.optionalAttrs (lang.excludeExtensions != [ ]) {
      exclude_extensions = lang.excludeExtensions;
    }
  ) cfg.languageOverlays;

  configFile = tomlFormat.generate "aifed-config.toml" {
    lsp = lspEntries;
    language = languageEntries;
  };
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

    languageOverlays = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule {
          options = {
            language = lib.mkOption {
              type = lib.types.str;
              description = "Language identifier this overlay targets.";
            };

            additionalExtensions = lib.mkOption {
              type = lib.types.listOf lib.types.str;
              default = [ ];
              description = "Extensions to add to the language's grammar defaults.";
            };

            excludeExtensions = lib.mkOption {
              type = lib.types.listOf lib.types.str;
              default = [ ];
              description = "Default extensions to remove for this language.";
            };
          };
        }
      );
      default = { };
      description = ''
        `[[language]]` extension overlays, keyed by name. Layer on the in-code
        grammar defaults:
        effective = (defaults ∪ additionalExtensions) − excludeExtensions.
        Empty by default; grammar languages (rust, markdown) resolve from code
        without any overlay. Use this to add extensions to a grammar language,
        or to declare a config-only language (one aifed can't outline).
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile."aifed/config.toml".source = configFile;
  };
}
