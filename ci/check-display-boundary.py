#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

"""Reject runtime dependencies reachable from the ADR-078 display crate."""

import json
import sys
from pathlib import Path

DISPLAY = "vauchi-cli-display"
ALLOWED_VAUCHI = {DISPLAY, "vauchi-contract"}
RUNTIMES = {f"{platform}-{kind}" for platform in ("linux", "apple", "android", "windows", "macos") for kind in ("hw", "app")}


def check_boundary(metadata):
    packages = {package["id"]: package["name"] for package in metadata["packages"]}
    roots = [member for member in metadata["workspace_members"] if packages[member] == DISPLAY]
    if len(roots) != 1:
        raise ValueError(f"workspace must contain exactly one {DISPLAY}")
    if not metadata.get("resolve"):
        raise ValueError("Cargo metadata must include the resolved dependency graph")
    nodes = {node["id"]: node["dependencies"] for node in metadata["resolve"]["nodes"]}
    pending = [(roots[0], [])]
    visited = set()
    while pending:
        package_id, path = pending.pop()
        if package_id in visited:
            continue
        if package_id not in packages or package_id not in nodes:
            raise ValueError(f"incomplete Cargo graph at {package_id}")
        visited.add(package_id)
        name = packages[package_id]
        path = [*path, name]
        if name in RUNTIMES or name.startswith("vauchi-") and name not in ALLOWED_VAUCHI:
            raise ValueError("display depends on runtime: " + " -> ".join(path))
        pending.extend((dependency, path) for dependency in nodes[package_id])


def main():
    if len(sys.argv) != 2:
        print("usage: check-display-boundary.py CARGO_METADATA.json", file=sys.stderr)
        return 2
    try:
        check_boundary(json.loads(Path(sys.argv[1]).read_text(encoding="utf-8")))
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"display-boundary: {error}", file=sys.stderr)
        return 1
    print(f"display-boundary: OK ({DISPLAY})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
