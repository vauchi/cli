# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

import yaml


class CiLoader(yaml.SafeLoader):
    pass


CiLoader.add_constructor("!reference", lambda loader, node: loader.construct_sequence(node))
ROOT = Path(__file__).resolve().parents[2]
JOBS = yaml.load((ROOT / ".gitlab-ci.yml").read_text(), Loader=CiLoader)
JOB = JOBS["publish:package:cli"]
PACKAGE = JOB["variables"]["PACKAGE_NAME"]
BINARY = JOB["variables"]["BINARY_NAME"]
BASE = "https://registry.invalid/api/v4/projects/123/packages/generic/" + PACKAGE
SHA = "1" * 40
TAG = "v9.8.7"


def urls(version):
    return [f"{BASE}/{version}/{BINARY}", f"{BASE}/{version}/{BINARY}.sha256"]


class PackagePublicationTest(unittest.TestCase):
    def run_job(self, tag, previous=(), fail=False):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            (work / "target/release").mkdir(parents=True)
            (work / "target/release/vauchi").write_text("test artifact\n")
            tools = work / "tools"
            tools.mkdir()
            log = work / "uploads"
            log.write_text("".join(url + "\n" for url in previous))
            curl = tools / "curl"
            curl.write_text('''#!/bin/sh
set -eu
for argument do url=$argument; done
if [ "${FAIL_UPLOAD:-0}" = 1 ] || grep -Fxq "$url" "$UPLOAD_LOG"; then
    echo 'registry rejected upload' >&2
    exit 22
fi
printf '%s\\n' "$url" >> "$UPLOAD_LOG"
''')
            curl.chmod(0o755)
            environment = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                               UPLOAD_LOG=str(log), FAIL_UPLOAD="1" if fail else "0",
                               CI_API_V4_URL="https://registry.invalid/api/v4", CI_PROJECT_ID="123",
                               CI_JOB_TOKEN="test-token", CI_COMMIT_SHA=SHA, CI_COMMIT_TAG=tag,
                               **JOB["variables"])
            result = subprocess.run(["sh", "-c", "\n".join(JOB["script"])], cwd=work,
                                    env=environment, text=True, capture_output=True)
            return result, log.read_text().splitlines()

    # @internal — main publishes exactly its immutable commit artifact pair.
    def test_main_publishes_commit_files(self):
        result, uploads = self.run_job("")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(uploads, urls(SHA))

    # @internal — a release follows main at the same SHA without reuploading it.
    def test_tag_publishes_tag_files_when_main_already_published_sha(self):
        result, uploads = self.run_job(TAG, previous=urls(SHA))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(uploads, urls(SHA) + urls(TAG))

    # @internal — an existing tag artifact must not be overwritten or ignored.
    def test_duplicate_tag_upload_still_fails(self):
        result, uploads = self.run_job(TAG, previous=urls(SHA) + urls(TAG))
        self.assertEqual(result.returncode, 22)
        self.assertEqual(uploads, urls(SHA) + urls(TAG))

    # @internal — publication failures must fail the job instead of hiding a release gap.
    def test_registry_failure_still_fails_main_and_tag(self):
        for tag in ["", TAG]:
            with self.subTest(tag=tag):
                result, uploads = self.run_job(tag, fail=True)
                self.assertEqual(result.returncode, 22)
                self.assertEqual(uploads, [])


if __name__ == "__main__":
    unittest.main()
