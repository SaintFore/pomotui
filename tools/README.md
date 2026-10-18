# Repository checks

From the repository root, run workflow validation:

```sh
python3 tools/lint-workflows.py
```

The bootstrap requires Python 3, Go 1.21 or newer, and `patch`. It selects
Go 1.25.0, downloads actionlint v1.7.12 through Go's verified module mechanism,
checks the pinned module checksum, and applies `actionlint-queue.patch` without
fuzz. The cache key includes the bootstrap, patch digest, and toolchain. Nothing depends on
the temporary tools built during the original release implementation.

The patch adds GitHub's documented `concurrency.queue: single|max` values and
rejects literal `queue: max` combined with `cancel-in-progress: true`.
[GitHub concurrency reference](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency).
Remove the patch when upgrading to an upstream release that implements this
schema, retaining the public validator regression tests.

Pass workflow files as arguments to validate isolated examples. Both optional
shellcheck and pyflakes integrations are disabled explicitly so installation of
unrelated host tools does not change this command's result; workflow syntax and
expression checks remain active. CI runs this command and its behavior tests.
Downloaded modules and binaries live beneath `$XDG_CACHE_HOME/pomotui`, falling
back to `~/.cache/pomotui`.

Before expensive package builds, run the release boundary tests at both versions:

```sh
python3 tests/release/check-boundaries.py
python3 tests/release/check-boundaries.py --version 0.0.0
```

The alternate version runs in a disposable copy of the source workspace. It
updates the workspace manifest and local member entries in Cargo.lock, leaves
third-party dependency versions unchanged, and never changes the source tree.
Candidate workflows run these checks before starting package builds. Candidate
uploads use `release.py pack` so transport follows the verified manifest rather
than a second hand-maintained list of artifact paths.
