# Releases and distribution

[Developer guide](index.md) · [Changelog](https://github.com/ecmel/pliq/blob/main/CHANGELOG.md)

## Distribution model

pliq is distributed under the MIT [LICENSE](https://github.com/ecmel/pliq/blob/main/LICENSE).
Keep `publish = false` in [Cargo.toml](https://github.com/ecmel/pliq/blob/main/Cargo.toml). Routine contribution
work does not publish source or package artifacts.

Binary releases are triggered by pushing a `vx.y.z` tag. The version in the
tag must match `Cargo.toml`. The workflow creates or updates a GitHub release
using the corresponding changelog section as its notes.

## Prepare a version

1. Finish the intended changes and [validation](testing.md).
2. Update the package version and its lockfile entry together.
3. Move implemented entries from `[Unreleased]` into
   `## [x.y.z] - YYYY-MM-DD`, leaving `[Unreleased]` in place.
4. Update changelog link references: `[Unreleased]` compares the new tag with
   `HEAD`, each release compares with its predecessor, and the first links to
   its tag. Use the repository's actual remote URL.
5. Review the release changes before creating and pushing the matching tag.

Within a changelog section, use Added, Changed, Deprecated, Removed, Fixed, and
Security in that order when applicable. Record implemented behavior, not plans.

## Workflow behavior

The [release workflow](https://github.com/ecmel/pliq/blob/main/.github/workflows/release.yml) runs on version-tag
pushes and builds this matrix:

| Platform       | Target                     | Archive   |
| -------------- | -------------------------- | --------- |
| Linux x86-64   | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| Linux arm64    | `aarch64-unknown-linux-gnu` | `.tar.gz` |
| Windows x86-64 | `x86_64-pc-windows-msvc`   | `.zip`    |
| Windows arm64  | `aarch64-pc-windows-msvc`  | `.zip`    |
| macOS x86-64   | `x86_64-apple-darwin`      | `.tar.gz` |
| macOS arm64    | `aarch64-apple-darwin`     | `.tar.gz` |

Linux arm64 uses `ubuntu-24.04-arm` and Windows arm64 uses `windows-11-arm`,
so both architectures build and run tests on native ARM64 runners.
macOS x86-64 uses the Intel runner `macos-15-intel`.

Each build installs stable Rust, runs release tests with the lockfile, builds
the binary, and packages it with `LICENSE` and `README.md`. The current archives
do not bundle the full `docs/` tree. The normal workflow tests skip ignored tests.

The publishing job checks tag/version agreement, requires a nonempty matching
changelog section, collects the archives, and produces `SHA256SUMS.txt`. It
creates the release if absent or uploads replacement assets to an existing
release, allowing a partial workflow to be rerun. For an existing release it
uploads assets without rewriting the release notes.

Archive names carry the target triple but no version, for example
`pliq-aarch64-apple-darwin.tar.gz`. GitHub serves
`https://github.com/ecmel/pliq/releases/latest/download/<name>` from the most
recent published release, skipping drafts and prereleases, so
[Getting started](../user/getting-started.md) links to the archives and
`SHA256SUMS.txt` that way. Renaming an archive or adding a target breaks or
omits those links; update that page's download table with the matrix.
