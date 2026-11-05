# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

import runpy
import unittest
from pathlib import Path

CHECK = runpy.run_path(str(Path(__file__).resolve().parents[1] / "check-display-boundary.py"))["check_boundary"]


def graph(extra=()):
    names = ["vauchi-cli-display", "vauchi-contract", "console", "vauchi-app", *extra]
    return {
        "packages": [{"id": name, "name": name} for name in names],
        "workspace_members": ["vauchi-cli-display", "vauchi-app"],
        "resolve": {"nodes": [
            {"id": name, "dependencies": ["vauchi-contract", "console"] if name == "vauchi-cli-display" else []}
            for name in names
        ]},
    }


class DisplayBoundaryTest(unittest.TestCase):
    # @internal — ADR-078 display dependency boundary.
    def test_contract_and_terminal_dependencies_pass_with_app_elsewhere_in_workspace(self):
        CHECK(graph())

    # @internal — ADR-078 display dependency boundary.
    def test_direct_runtime_dependency_is_rejected(self):
        for name in ["vauchi-core", "vauchi-app", "vauchi-hardware", "linux-hw", "apple-hw"]:
            with self.subTest(name=name):
                data = graph(() if name == "vauchi-app" else (name,))
                data["resolve"]["nodes"][0]["dependencies"].append(name)
                with self.assertRaisesRegex(ValueError, name):
                    CHECK(data)

    # @internal — ADR-078 display dependency boundary.
    def test_transitive_runtime_dependency_reports_the_path(self):
        data = graph(("vauchi-core",))
        data["resolve"]["nodes"][2]["dependencies"] = ["vauchi-core"]
        with self.assertRaisesRegex(ValueError, "vauchi-cli-display -> console -> vauchi-core"):
            CHECK(data)

    # @internal — Cargo resolves renamed dependencies to package IDs.
    def test_build_and_dev_runtime_packages_are_rejected_too(self):
        for kind in ["build", "dev"]:
            with self.subTest(kind=kind):
                data = graph(("vauchi-core",))
                root = data["resolve"]["nodes"][0]
                root["dependencies"].append("vauchi-core")
                root["deps"] = [{"name": "core", "pkg": "vauchi-core", "dep_kinds": [{"kind": kind, "target": None}]}]
                with self.assertRaisesRegex(ValueError, "vauchi-core"):
                    CHECK(data)

    # @internal — Cargo resolves renamed dependencies to package IDs.
    def test_renamed_dependency_is_checked_by_its_package_name(self):
        data = graph(("registry+example#runtime-alias@1.0.0",))
        data["packages"][-1]["name"] = "vauchi-core"
        data["resolve"]["nodes"][0]["dependencies"].append(data["packages"][-1]["id"])
        with self.assertRaisesRegex(ValueError, "vauchi-core"):
            CHECK(data)

    # @internal — Incomplete metadata must not silently pass the gate.
    def test_missing_resolve_or_reachable_node_is_rejected(self):
        for missing in ["resolve", "node"]:
            with self.subTest(missing=missing):
                data = graph()
                if missing == "resolve":
                    data["resolve"] = None
                else:
                    data["resolve"]["nodes"].pop(1)
                with self.assertRaises(ValueError):
                    CHECK(data)

    # @internal — Check the workspace's display package, not a stale name.
    def test_missing_display_workspace_member_is_rejected(self):
        data = graph()
        data["workspace_members"].remove("vauchi-cli-display")
        with self.assertRaisesRegex(ValueError, "workspace"):
            CHECK(data)


if __name__ == "__main__":
    unittest.main()
