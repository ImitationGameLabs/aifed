# Installation Guide

This guide covers how to install aifed on your system.

## Overview

aifed can be installed on various operating systems. Choose the installation method that matches your system:

- [Install with Cargo (Git)](#install-with-cargo-git) — Recommended for Linux, macOS, and other Unix-like systems
- [Build from Source](#build-from-source) — For local development or manual installation
- [Installing on NixOS](#installing-on-nixos) — For NixOS users

---

## Install with Cargo (Git)

This is the recommended installation method for Linux, macOS, and other Unix-like systems.

### Prerequisite

Install the Rust toolchain first if you do not already have it.

### Install

```bash
cargo install --git https://github.com/ImitationGameLabs/aifed aifed --locked
```

This installs the `aifed` binary into Cargo's bin directory (typically `~/.cargo/bin`).

### Verify the installation

```bash
aifed --version
```

### Next step

Then proceed to [Post-Installation](#post-installation-configure-your-ai-agent).

---

## Build from Source

This section covers local builds when you want to work from a checkout directly.

### Step 1: Clone the Repository

```bash
git clone https://github.com/ImitationGameLabs/aifed.git
cd aifed
```

### Step 2: Build the Binary

```bash
cargo build --release
```

The compiled binary will be available at `target/release/aifed`.

### Step 3: Install to System

Copy the binary to your system's binary directory:

```bash
# Requires sudo privileges
sudo cp target/release/aifed /usr/local/bin/
```

### Step 4: Verify the installation

```bash
aifed --version
```

### Step 5: Configure Your AI Agent

Then proceed to [Post-Installation](#post-installation-configure-your-ai-agent).

---

## Installing on NixOS

This section covers installing aifed on NixOS using flakes.

### Prerequisites

Ensure you have flakes enabled in your NixOS configuration:

```nix
# configuration.nix
nix.settings.experimental-features = [ "nix-command" "flakes" ];
```

### Step 1: Add Flake Input

Add aifed to your flake inputs. This can be done in either your NixOS configuration or home-manager flake:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    aifed = {
      url = "https://github.com/ImitationGameLabs/aifed";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
}
```

### Step 2: Install the Package

**Option A: Via Home Manager**

```nix
{
  home.packages = [
    inputs.aifed.packages.${system}.aifed
  ];
}
```

**Option B: Via NixOS Configuration**

```nix
{
  environment.systemPackages = [
    inputs.aifed.packages.${system}.aifed
  ];
}
```

### Step 3: Apply Changes

Rebuild your configuration:

```bash
# For NixOS
sudo nixos-rebuild switch

# For Home Manager
home-manager switch
```

Verify the installation:

```bash
aifed --version
```

### Step 4: Configure Your AI Agent

Then proceed to [Post-Installation](#post-installation-configure-your-ai-agent).

---

## Post-Installation: Configure Your AI Agent

To get the best experience with aifed, configure your AI agent to use aifed for file operations. Any AI agent can become capable of code editing with the right tools. Copy the following prompt to your AI agent:

```md
Please add the following instructions to your persistent memory:

"Before using any aifed command, you MUST run `aifed --skill` to get the complete usage guide. For all file operations:

1. Use `aifed read` and `aifed edit` instead of `cat`, `sed`, or built-in file tools
2. This applies even when only reading files, unless you are certain you are just exploring and will not make any modifications
3. When context is compressed, preserve the complete `aifed --skill` output

Why not mix tools? Alternating between aifed and built-in file tools breaks integrity checks on both sides. Modifications made through one tool are invisible to the other, causing verification failures, requiring re-reads, and wasting tokens.

This ensures better tracking, verification, and recovery capabilities."
```

Different AI agents have different ways to manage persistent memory—refer to your agent's documentation for the specific method.
