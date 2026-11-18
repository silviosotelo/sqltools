# Editor tabs and split DDL — implementation plan

**Goal:** `Ctrl+F12` on a package or type opens its spec and its body as two
separate editor tabs (like the original loads both, but never mixed into one
buffer), and every document the app opens lives in its own tab.

**Architecture:** `App.tsx` keeps a `docs: EditorDoc[]` list plus
`activeDocId`; the values every existing flow already uses (`sql`, `cleanSql`,
`fileEol`, `cleanEol`, `filePath`) are derived from the active document and the
old setters become patches on it. `catalog::load_ddl` already normalises
`PACKAGE BODY`/`TYPE BODY` to the `DBMS_METADATA` spelling, so the frontend asks
for the body and skips it when Oracle reports that the object has none.

**Tech Stack:** React 19 + TypeScript, CodeMirror 6, Tauri v2, Rust.

**Spec:** user direction 2026-11-18 — "tiene que ser como el original, pero
separar por ventanas mejor, así no sale mezclado".

**Status:** implemented and verified 2026-11-18 (evidence below).

## Global Constraints

- UI strings only through `i18n.tsx` (es/en); code, comments and errors in English.
- Colours only through the tokens in `web/DESIGN.md`.
- `Ctrl+F12` keeps loading DDL for every explorer type: table, view, package,
  procedure, function, trigger, sequence, type, synonym.
- The active tab is the one Execute, Format, Save, Export and History act on.

## Review Focus

- A package without a body must not show an error or an empty tab.
- Switching tabs must not lose the other tab's unsaved text.
- Closing the last tab leaves the app usable (one empty `SQL` tab).
- Loading the same object twice must not grow duplicate tabs.
- `Ctrl+F12` on a table adds a tab instead of overwriting what the user typed.

---

### Task 1: Document list in `App.tsx` — done

**Files:**
- Modify: `rust/web/src/App.tsx`
- Modify: `rust/web/src/styles.css`

**Interfaces (as built):**
- `type EditorDoc = { id, title, key, sql, cleanSql, eol, cleanEol, path }`.
- `openDoc(title, text, { key?, path?, eol? })` dedupes on `key` (defaults to
  the path, else `title:<title>`), returns the id.
- DDL tabs use `ddl:<owner>.<name>.<type>` and `…:body`, so the same object in
  two schemas never collides and reloading replaces instead of duplicating.
- `closeDoc(id)` activates the left neighbour (else the right one); the last tab
  resets to one empty `SQL` tab. `resetDocs()` backs `File.CloseAll`.
- `File.Close`/`CloseAll` confirm when the active tab is dirty; closing a single
  tab does not ask (DDL is regenerable, files reopen).
- Workspace copy/paste/save/save-as/load serialise every tab
  (`toWorkspace`/`restoreWorkspace`), which the `Workspace.files[]` type already
  supported.
- `saveFile` captures the target document at entry so a Save-As dialog cannot
  write one tab's text and clear another tab's dirty flag.
- Drag and drop, Open, Open-at-cursor, `Script.LoadDDLWithDbmsMetadata` and
  "New from template" open a tab instead of overwriting the active buffer.
- Package/type body: any error other than `ORA-31603` is surfaced; `ORA-31603`
  means "no body" and is silent (the discriminator the original used).
- Tab bar: title with `*` when dirty, close button, click activates and focuses
  the editor, connection chip moved to the right end of the bar.
- CodeMirror: switching documents replaces the buffer with
  `Transaction.addToHistory.of(false)` and resets the history compartment, so
  `Ctrl+Z` never walks into another tab's text.

### Task 2: Proof — done

- `npm --prefix rust/web run build` → 0.
- Installed app, `Ctrl+F12` on `ANAMNESIS.AS_PDF3_MOD`: tabs `SQL`,
  `ANAMNESIS.AS_PDF3_MOD (PACKAGE)` (active, `CREATE OR REPLACE EDITIONABLE
  PACKAGE …`) and `ANAMNESIS.AS_PDF3_MOD (PACKAGE BODY)`
  (`CREATE OR REPLACE EDITIONABLE PACKAGE BODY …`); reloading the same package
  keeps three tabs.
- Table `A1` adds `ANAMNESIS.A1 (TABLE)` with `CREATE TABLE a1 …`; editing marks
  that tab with `*`; `F5` runs the active tab (`1 fila · 1 ms`).
- Workspace round-trip: save → `files: 4`; restore → four tabs with their titles
  and the first active. `File.New` adds a `SQL` tab; `File.CloseAll` leaves one;
  the close button removes the active tab.
- Undo: inside a tab `Ctrl+Z` reverts the typing; after switching, `Ctrl+Z`
  leaves the other tab untouched.
- `cargo fmt --check`, `cargo clippy -D warnings`,
  `cargo test --workspace --exclude sqlt-desktop` → clean; NSIS installer
  rebuilt and reinstalled; screenshot `docs/screenshots/editor-tabs.png`.
