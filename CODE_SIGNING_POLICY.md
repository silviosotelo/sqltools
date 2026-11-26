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

- **Windows: self-signed.** The installer, the MSI and the app binary are
  signed with a generated code-signing certificate (locally, and in CI run
  `45812702256` where `Get-AuthenticodeSignature` lists both
  `SQLTools_0.1.1_x64-setup.exe` and the `.msi` as signed by
  `CN=SQLTools Self-Signed`; the local one is
  (`CN=SQLTools Self-Signed`, thumbprint `3B9C9F1B…F7`, valid until 2029). The
  app binary is signed **before** bundling (`tauri build --no-bundle` →
  `signtool sign` → `tauri bundle`), so the installed
  `%LOCALAPPDATA%\SQLTools\sqlt-desktop.exe` carries the signature too.
  `Get-AuthenticodeSignature` reports `Status: UnknownError` because the root is
  not trusted outside this machine — that is what self-signing means: the
  binaries are signed, but SmartScreen will still warn until a CA-issued or
  SignPath certificate is used.
- **macOS: ad-hoc signature (CI).** When no Apple identity secret exists the
  workflow sets `APPLE_SIGNING_IDENTITY=-` and Tauri signs the bundle ad-hoc.
  A throwaway self-signed certificate was tried first and made `codesign` wait
  forever for a private-key access prompt on the runner (two runs of 40
  minutes), so it was dropped: an ad-hoc signature needs no keychain, passes
  `codesign --verify --deep --strict` (`valid on disk`, `satisfies its
  Designated Requirement`, `TeamIdentifier=not set`) and Gatekeeper rejects it
  exactly like it would reject a self-signed one (`spctl: rejected`, no
  notarization). Verified in CI run `45812702256`.
- The updater signatures in `latest.json` are a different mechanism; they work
  and are unaffected.
- Linux packages are not distro-signed; the `.sig` files in the release are
  Tauri updater signatures.

### What a trusted signature would still need

1. **SignPath** (or another CA): an approved organization/project plus the
   signing policy and artifact configuration, stored as `SIGNPATH_API_TOKEN`,
   `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`,
   `SIGNPATH_SIGNING_POLICY_SLUG` and `SIGNPATH_ARTIFACT_CONFIGURATION_SLUG`
   repository secrets. A real `WINDOWS_CERTIFICATE` secret is also honoured by
   the workflow (it skips the self-signed certificate when present).
2. **Apple**: `APPLE_ID`, `APPLE_PASSWORD` (app-specific password) and
   `APPLE_TEAM_ID` as repository secrets; the Developer ID certificate is
   already configured. With those, the workflow stops generating the
   self-signed identity and Tauri notarizes the bundle.

## Privacy policy

See [PRIVACY.md](PRIVACY.md).
