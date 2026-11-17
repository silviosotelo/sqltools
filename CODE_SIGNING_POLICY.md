# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by
[SignPath Foundation](https://signpath.org).

## Scope

This policy covers the Windows installers and executables built from this repository
by the GitHub Actions workflow `.github/workflows/rust-ci.yml` (Tauri bundles: `.msi`
and `-setup.exe`). macOS and Linux artifacts are out of scope: macOS distribution
requires a separate Apple Developer ID signature and notarization.

## Team roles

The project currently has a single maintainer. The roles are separated by process:

- Authors/committers: [@silviosotelo](https://github.com/silviosotelo)
- Reviewers: [@silviosotelo](https://github.com/silviosotelo) (changes are reviewed
  before merge; external pull requests require a review)
- Approvers: [@silviosotelo](https://github.com/silviosotelo) (every signing request is
  approved manually after checking the commit and the CI run)

If more maintainers join, this section will list the GitHub teams used for each role.

## Release and signing process

1. A release is built by GitHub Actions from a tagged commit on `main`.
2. The workflow runs `cargo fmt`, Clippy, the workspace tests and, when database
   credentials are available, the Oracle integration tests before producing the Tauri
   bundles.
3. The maintainer checks the commit, the workflow run and the artifacts, then approves
   the signing request in SignPath.
4. SignPath signs the Windows artifacts with the SignPath Foundation certificate.
   SHA-256 checksums are published with the release.

The signed packages may contain unsigned upstream open-source components (for example
WebView2 or system libraries), as allowed by the SignPath Foundation policy.

## Artifact metadata

Signed Windows binaries set the product name to `SQLTools` and the product version to
the release version from `rust/desktop/tauri.conf.json`, enforced by the SignPath
artifact configuration.

## Privacy policy

See [PRIVACY.md](PRIVACY.md).
