# Agent Actions

Step-by-step guides for AI agents to perform common project tasks.

## Purpose

This directory contains **action wizards** - structured documents that guide AI through multi-step operations. Each document:

- Provides a clear sequence of steps
- Includes check-before-act patterns
- Handles edge cases and common scenarios
- Ensures consistency across sessions

## Available Actions

| Document | Purpose |
| -------- | ------- |
| [add-workspace-member.md](add-workspace-member.md) | Add a new Rust crate to the workspace |

## When to Use

AI should follow these guides when:
- Performing infrastructure changes (adding crates, updating configs)
- Multi-step operations that need consistency
- Tasks with multiple edge cases to handle

## For Humans

If you need to understand what AI is doing:
1. Read the relevant action document
2. Each step explains what it checks and what it does
3. Follow the verification steps at the end
