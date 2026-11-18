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

## Current status (verified 2026-11-18)

- **Windows: unsigned.** `Get-AuthenticodeSignature` on the freshly built
  `SQLTools_0.1.1_x64-setup.exe` and on the installed
  `%LOCALAPPDATA%\SQLTools\sqlt-desktop.exe` returns `Status: NotSigned`.
  The SignPath application is still under review and the workflow has no
  SignPath step yet. The updater signatures in `latest.json` are a different
  mechanism and do work (a 0.1.0 build updated itself to 0.1.1).
- **macOS: signed, not notarized.** The bundle carries a Developer ID
  signature (`codesign --verify` passes, TeamIdentifier `7XF526FBNY`) but
  `spctl --assess` reports `rejected: Unnotarized Developer ID`. The workflow
  already forwards `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` and `APPLE_TEAM_ID`
  to the bundle job, so the notarization step starts as soon as the Apple
  credentials exist as repository secrets; that run has not happened yet.
- Linux packages are not distro-signed; the `.sig` files in the release are
  Tauri updater signatures.

### What is still needed

1. **SignPath**: an approved organization/project plus the signing policy and
   artifact configuration, stored as `SIGNPATH_API_TOKEN`,
   `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`,
   `SIGNPATH_SIGNING_POLICY_SLUG` and `SIGNPATH_ARTIFACT_CONFIGURATION_SLUG`
   repository secrets. The workflow step that submits the signing request is
   not written yet because its inputs cannot be verified without the account.
2. **Apple**: `APPLE_ID`, `APPLE_PASSWORD` (app-specific password) and
   `APPLE_TEAM_ID` as repository secrets; the Developer ID certificate is
   already configured. The next tagged build should then notarize and
   `spctl --assess` should accept it (not verified yet).

## Privacy policy

See [PRIVACY.md](PRIVACY.md).
