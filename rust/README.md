# SQLTools Rust

Rust rewrite of SQLTools (Oracle client for PL/SQL/APEX developers), with a web UI
(Tauri v2 + React). Target platforms: Windows x64, macOS Intel/Apple Silicon and
Linux x64.

Status: **P4/P5 (commands, workspaces, packaging)**. The C++/Qt port remains
untouched at the repository root (branch `main`); this branch (`rust`) contains
the new implementation.

The app can store connections (metadata in JSON, passwords only in the OS
keyring), connect, edit SQL with CodeMirror (folding, bookmarks, autocomplete),
execute with streaming results and a virtualized grid, cancel, commit/rollback,
browse history, plan and session statistics, export CSV, explore objects, inspect
columns and load DDL through `DBMS_METADATA`. UI strings go through i18n (es/en).

## Structure

```
rust/
  crates/sqlt-core/    # pure logic (statement splitter, formatting, binds...)
  crates/sqlt-oracle/  # Oracle driver (ODPI-C), sessions, batches, cancellation
  crates/sqlt-app/     # connection store, keyring, application state
  crates/sqlt-cli/     # headless harness: diag, query, bench
  desktop/             # Tauri v2 shell and commands
  web/                 # React + TypeScript + Vite frontend
```

## Requirements

- Stable Rust (MSVC host on Windows; Xcode CLT on macOS; build-essential on Linux).
- Node.js 22+ and npm.
- Tauri v2: WebView2 on Windows, WebKitGTK on Linux.
- Oracle Instant Client **only for the database layer**. It is not included in
  the repository because of its OTN license.

## Conventions

- **All code in English**: identifiers, functions, tests, comments, error
  messages and CLI output.
- **UI strings via i18n** (`rust/web/src/i18n.tsx`): no hardcoded text; `es`/`en`
  dictionaries.
- Commits in English.

## CI

Windows x64, Linux x64 and macOS Intel (`macos-15-intel`). The GitHub arm64
runner is not available to this account (jobs stay queued and are cancelled
after 15 minutes), so Apple Silicon is verified with a self-hosted runner or a
local build; the Rust/Tauri code is the same.

## Commands

```bash
# from rust/
cargo fmt --all --check
cargo clippy --workspace --exclude sqlt-desktop --all-targets -- -D warnings
cargo test --workspace --exclude sqlt-desktop

# Oracle (see crates/sqlt-oracle/README.md for environment variables)
cargo run -p sqlt-cli -- diag

# frontend
npm --prefix web install
npm --prefix web run build

# desktop app
cargo build -p sqlt-desktop

# bundle (from rust/desktop; the frontend must be built first)
npx @tauri-apps/cli@^2 build
# artifacts: target/release/bundle/{msi,nsis} (Windows), dmg/app (macOS), deb/appimage (Linux)
```

Signing/notarization and per-OS installers are completed in P5; unsigned
bundling is already verified on Windows in P0.

## Packaging, updates and migration

- CI builds unsigned bundles on Windows x64, macOS Intel and Linux x64
  (`msi`/`nsis`, `dmg`/`app`, `deb`/`AppImage`).
- The updater plugin is wired (`check_updates`), but it needs a real public key
  and an update endpoint/release host. Signing/notarization needs certificate
  identities. Those are external secrets, not repository code.
- Legacy SQLTools Mac connection profiles can be imported from the
  `connections/list` JSON array (tag/user/alias/TNS/direct/mode/read-only);
  passwords are never imported.

## License

GPLv3. See `LICENSE` at the repository root and `NOTICE` in this directory.
