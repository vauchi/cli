<!-- SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# CLAUDE.md - vauchi-cli

> **Inherits**: See [CLAUDE.md](../CLAUDE.md) for project-wide rules.

Command-line interface for testing and development.

## Component-Specific Rules

- CLI is for testing/dev, not end-user facing
- Depends on `vauchi-core`

## Commands

```bash
cargo run -p vauchi-cli -- init "Name"      # Initialize identity
cargo run -p vauchi-cli -- --help           # Show help
cargo test -p vauchi-cli                    # Run tests
```

## Usage

Primarily used for manual testing of core functionality without mobile/desktop UI.
