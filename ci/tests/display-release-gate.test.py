# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

import re
import unittest
from pathlib import Path

import yaml


class CiLoader(yaml.SafeLoader):
    pass


CiLoader.add_constructor("!reference", lambda loader, node: loader.construct_sequence(node))
JOBS = yaml.load((Path(__file__).resolve().parents[2] / ".gitlab-ci.yml").read_text(), Loader=CiLoader)


def required_ancestors(job, visited=None):
    visited = set() if visited is None else visited
    for entry in JOBS[job].get("needs", []):
        dependency = {"job": entry} if isinstance(entry, str) else entry
        name = dependency["job"]
        if dependency.get("optional", False) or name in visited:
            continue
        if JOBS[name].get("allow_failure", False):
            continue
        visited.add(name)
        required_ancestors(name, visited)
    return visited


def condition_matches(condition, variables):
    equal = re.fullmatch(r'\$(\w+) == (?:"([^"]*)"|\$(\w+))', condition)
    if equal:
        left, literal, right = equal.groups()
        return variables[left] == (variables[right] if right else literal)
    regex = re.fullmatch(r"\$(\w+) =~ /(.+)/", condition)
    if regex:
        variable, pattern = regex.groups()
        return re.search(pattern, variables[variable]) is not None
    raise ValueError(f"unsupported display-job rule: {condition}")


class DisplayReleaseGateTest(unittest.TestCase):
    # @internal — ADR-078: publication must wait for display conformance.
    def test_every_package_publisher_requires_the_display_gate(self):
        publishers = [name for name in JOBS if name.startswith("publish:package:")]
        self.assertTrue(publishers, "no package publisher found")
        for publisher in publishers:
            with self.subTest(publisher=publisher):
                self.assertIn("contract:replay", required_ancestors(publisher))

    # @internal — A tag pipeline has no CI_COMMIT_BRANCH.
    def test_stable_tags_select_a_blocking_display_job(self):
        job = JOBS["contract:replay"]
        for tag in ["v0.1.0", "v12.34.56"]:
            with self.subTest(tag=tag):
                variables = {"CI_PIPELINE_SOURCE": "push", "CI_COMMIT_BRANCH": "",
                             "CI_DEFAULT_BRANCH": "main", "CI_COMMIT_TAG": tag}
                selected = next((rule for rule in job["rules"]
                                 if "if" not in rule or condition_matches(rule["if"], variables)), None)
                self.assertIsNotNone(selected, "stable tag omits the display job")
                self.assertNotEqual(selected.get("when"), "never")
                self.assertFalse(selected.get("allow_failure", job.get("allow_failure", False)))

    # @internal — The gate's policy regression runs before its Rust work.
    def test_display_gate_requires_its_release_policy_check(self):
        self.assertIn("check:display-release", required_ancestors("contract:replay"))


if __name__ == "__main__":
    unittest.main()
