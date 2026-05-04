<!-- SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Vendored golden fixtures

Vendored from `core/vauchi-core/tests/fixtures/golden/` so the
CLI's `golden_snapshot_*` tests don't require `vauchi/core` to be
checked out as a sibling on the CI runner.

## Sync

```sh
just sync-golden-fixtures cli
```

Run from the workspace root after `core/`'s golden fixtures change.
The recipe replaces every file under this directory with the
matching file from `core/vauchi-core/tests/fixtures/golden/`.

## Drift

The vendored copies can drift from the source. Two ways drift
becomes visible:

1. A snapshot test in `src/ui/screen_renderer_tests.rs` fails after
   a `core` rev bump that changed the `ScreenModel` shape — the
   vendored fixture still has the old shape, deserialization or
   rendering diverges from the snapshot.
2. The sync recipe's diff (`git diff cli/tests/fixtures/golden/`
   after running it) is non-empty, meaning a `core`-side update
   wasn't propagated.

Either signal means: re-run the sync, then `cargo insta review` any
snapshot churn, then commit fixtures + snapshots together.
