# SQLTools Rust — P0: bootstrap + primer slice verificable

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. (Método elegido por el usuario: `/loop` → executing-plans.)

**Goal:** Dejar el fork con una rama `rust` que compile y pruebe un workspace Rust multiplataforma, un esqueleto Tauri + React y el partidor de sentencias portado con sus 19 casos originales.

**Architecture:** Monorepo bajo `rust/`: workspace Cargo con `sqlt-core` (lógica pura) y `sqlt-desktop` (Tauri v2); frontend en `rust/web` (React + Vite). El C++ actual no se toca.

**Tech Stack:** Rust stable (MSVC), Cargo workspace, Tauri v2, React 19 + TypeScript + Vite, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-11-16-rust-migration-design.md`

## Global Constraints

- GPLv3; se conserva `LICENSE` y se agrega atribución en `rust/NOTICE`.
- Todo el código nuevo vive bajo `rust/`; no se modifica `src/`, `ui/`, `CMakeLists.txt`.
- Sin credenciales ni rutas absolutas en el repo.
- Offsets de posición expuestos a la UI en **unidades UTF-16** (compatibles con CodeMirror y con el port C++/QString).
- Rust edition 2021; `cargo fmt` y `clippy -D warnings` obligatorios.
- Windows: toolchain MSVC (`x86_64-pc-windows-msvc`). Node 22 ya instalado.

## Review Focus

1. Posiciones UTF-16: un script con acentos o emoji antes del `;` debe devolver el mismo `start/end` que vería CodeMirror.
2. Script vacío o solo comentarios: `split` devuelve lista vacía, nunca una sentencia fantasma.
3. `q'[...]'` y comillas escapadas: un `;` dentro del literal no corta la sentencia.
4. PL/SQL: bloques `BEGIN/DECLARE` anidados y `CREATE ... PROCEDURE/PACKAGE/TRIGGER` terminados por `/` en línea sola.
5. Build multiplataforma: rutas relativas de Tauri (`../web/dist`) y CI sin rutas de macOS/Windows hardcodeadas.

---

### Task 1: Fork, rama y baseline de documentación

**Files:**
- Create: `docs/superpowers/specs/2026-11-16-rust-migration-design.md` (ya escrito)
- Create: `docs/superpowers/plans/2026-11-16-rust-p0-bootstrap.md` (este archivo)
- Modify: `.gitignore`

**Interfaces:**
- Produce: remoto `origin` = fork `silviosotelo/sqltools`, remoto `upstream` = `JavierC78Py/sqltools`, rama `rust` publicada.

- [ ] **Step 1: Crear el fork si no existe y reconfigurar remotos**

```powershell
gh repo view silviosotelo/sqltools --json name 2>$null
# si no existe:
gh repo fork JavierC78Py/sqltools --clone=false --remote=false
git remote rename origin upstream
git remote add origin https://github.com/silviosotelo/sqltools.git
git fetch origin
```

- [ ] **Step 2: Crear la rama `rust` y agregar los ignores**

Agregar al final de `.gitignore`:

```gitignore
# Rust / Tauri
/rust/target/
/rust/web/node_modules/
/rust/web/dist/
/rust/desktop/gen/
```

- [ ] **Step 3: Verificar que el C++ sigue intacto**

Run: `git status --short`
Expected: solo `.gitignore` y los dos documentos nuevos sin trackear; ningún archivo de `src/` o `ui/`.

- [ ] **Step 4: Commit y push de la rama**

```powershell
git checkout -b rust
git add .gitignore docs/superpowers/specs docs/superpowers/plans
git commit -m "docs: spec y plan inicial de la migración a Rust"
git push -u origin rust
```

---

### Task 2: Toolchain Rust

**Files:** ninguno del repo (solo entorno).

**Interfaces:**
- Produce: `rustc`, `cargo`, `rustfmt`, `clippy` disponibles; host `x86_64-pc-windows-msvc`.

- [ ] **Step 1: Instalar rustup si falta**

```powershell
$tmp = "$env:TEMP\rustup-init.exe"
Invoke-WebRequest -Uri https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe -OutFile $tmp
& $tmp -y --default-host x86_64-pc-windows-msvc --default-toolchain stable --profile minimal
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

- [ ] **Step 2: Instalar componentes y verificar**

```powershell
rustup component add clippy rustfmt
rustc -V
cargo -V
cargo clippy -V
```

Expected: versiones estables impresas; sin error de linker (MSVC 14.29 y Windows SDK 10.0.19041 ya están instalados).

---

### Task 3: Workspace + `sqlt-core::split` (TDD)

**Files:**
- Create: `rust/Cargo.toml`
- Create: `rust/crates/sqlt-core/Cargo.toml`
- Create: `rust/crates/sqlt-core/src/lib.rs`
- Create: `rust/crates/sqlt-core/src/split.rs`
- Create: `rust/crates/sqlt-core/tests/split_test.rs`
- Reference: `ui/SqlSplitter.cpp` (algoritmo a portar), `ui/SqlSplitter.h` (contrato), `tests/split_test.cpp` (19 casos)

**Interfaces:**
- Produce:

```rust
// SPDX-License-Identifier: GPL-3.0-only
pub struct Statement {
    pub start: usize,      // unidad UTF-16, primer carácter útil
    pub end: usize,        // unidad UTF-16, justo después del último carácter (sin ';' ni '/')
    pub plsql: bool,
    pub start_line: usize, // 1-based
    pub text: String,
}
pub fn split(script: &str) -> Vec<Statement>;
pub fn statement_at(stmts: &[Statement], position_utf16: usize) -> Option<usize>;
```

- [ ] **Step 1: Crear el workspace y el crate vacío**

`rust/Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/*"]
# Task 4 agrega "desktop" a members.
```

`rust/crates/sqlt-core/Cargo.toml`:

```toml
[package]
name = "sqlt-core"
version = "0.1.0"
edition = "2021"
```

- [ ] **Step 2: Escribir las pruebas que fallan**

En `rust/crates/sqlt-core/tests/split_test.rs`, una prueba por cada caso de `tests/split_test.cpp` (dos selects, `;` en texto, comilla escapada, comentarios de línea/bloque, `q'[ ]'`, bloque anónimo, if/loop/case, declare+begin, bloques anidados, create procedure con `/`, package y body, create table, create trigger, sin `;` final, comentarios al inicio, barra sola, vacío) **más `statement_at` con las 4 aserciones de `split_test.cpp:46`** y los casos de UTF-16, solo comentarios, CRLF, identificador entrecomillado y `q'{ }'`:

```rust
#[test]
fn offsets_son_utf16_con_acentos_y_emoji() {
    let sql = "select 'ñ😀' from dual;\nselect 2 from dual;";
    let st = sqlt_core::split(sql);
    assert_eq!(st.len(), 2);
    assert_eq!(&sql_utf16_slice(sql, st[0].start, st[0].end), "select 'ñ😀' from dual");
    assert_eq!(st[1].start_line, 2);
}

#[test]
fn solo_comentarios_no_produce_sentencias() {
    assert!(sqlt_core::split("-- hola\n/* x */\n").is_empty());
}
```

`sql_utf16_slice` es un helper local del test que convierte los offsets UTF-16 a la subcadena real (implementarlo con `encode_utf16`).

- [ ] **Step 3: Ejecutar y ver que fallan**

Run: `cargo test -p sqlt-core`
Expected: FAIL de compilación (`split` no existe).

- [ ] **Step 4: Portar el algoritmo de `ui/SqlSplitter.cpp`**

Reglas a preservar: `;` fuera de textos/comentarios; `q'[...]'`; comillas `''`; bloques `BEGIN/DECLARE` con conteo de `BEGIN/IF/LOOP/CASE` contra `END`; `CREATE [OR REPLACE] PROCEDURE/FUNCTION/PACKAGE/TRIGGER/TYPE` que termina solo en una línea con `/`; línea `/` sola cierra; comentarios al inicio no generan sentencia. Calcular `start/end/start_line` en UTF-16.

- [ ] **Step 5: Ejecutar y ver que pasan**

Run: `cargo test -p sqlt-core`
Expected: PASS (todas las pruebas: 19 casos del original + `statement_at` + UTF-16 + solo comentarios + CRLF + identificador entrecomillado + `q'{ }'`).

- [ ] **Step 6: fmt + clippy**

Run: `cargo fmt --all && cargo clippy -p sqlt-core --all-targets -- -D warnings`
Expected: sin salida de error.

- [ ] **Step 7: Commit**

```powershell
git add rust
git commit -m "feat(rust): workspace y partidor de sentencias con paridad de pruebas"
```

---

### Task 4: Esqueleto Tauri v2 + React

**Files:**
- Create: `rust/desktop/Cargo.toml`, `rust/desktop/build.rs`, `rust/desktop/src/main.rs`, `rust/desktop/tauri.conf.json`, `rust/desktop/capabilities/default.json`
- Create: `rust/web/package.json`, `rust/web/index.html`, `rust/web/tsconfig.json`, `rust/web/vite.config.ts`, `rust/web/src/main.tsx`, `rust/web/src/App.tsx`

**Interfaces:**
- Produce: `cargo build -p sqlt-desktop` compila; `rust/web/dist` generado por Vite; ventana Tauri con placeholder (sin diseño visual: lo define `@designer`).

- [ ] **Step 1: Crear el frontend mínimo**

`rust/web/package.json` con `react`, `react-dom`, `@tauri-apps/api`, y dev deps `vite`, `@vitejs/plugin-react`, `typescript`, `@types/react`, `@types/react-dom`. Scripts: `dev` (`vite --port 1420 --strictPort`) y `build` (`tsc -b && vite build`). `vite.config.ts` con `server.port = 1420`, `clearScreen: false`, `envPrefix: ["VITE_", "TAURI_"]`.

- [ ] **Step 2: Instalar y compilar el frontend**

Run: `npm --prefix rust/web install; npm --prefix rust/web run build`
Expected: `rust/web/dist/index.html` existe, sin errores de TypeScript.

- [ ] **Step 3: Crear el shell Tauri**

`rust/desktop/Cargo.toml` (con `license = "GPL-3.0-only"`): `tauri = { version = "2", features = [] }`, `serde`, `serde_json`; build-dep `tauri-build = "2"`. Agregar `"desktop"` a `members` del workspace. `build.rs`: `fn main() { tauri_build::build() }`. `main.rs`: builder mínimo con `tauri::generate_context!()`. `tauri.conf.json` con `build.frontendDist: "../web/dist"`, `build.devUrl: "http://localhost:1420"`, `build.beforeDevCommand: "npm --prefix ../web run dev"`, `bundle.active: false` en P0 (el empaquetado es P5), identifier `com.sqltools.rust`. `capabilities/default.json` con `"permissions": ["core:default"]`.

- [ ] **Step 4: Compilar el desktop**

Run: `cargo build -p sqlt-desktop` (desde `rust/`)
Expected: binario en `rust/target/debug/`; sin errores de WebView2 ni de linking.

- [ ] **Step 5: Commit**

```powershell
git add rust
git commit -m "feat(rust): esqueleto Tauri v2 + React sobre el workspace"
```

---

### Task 5: CI multiplataforma y atribución

**Files:**
- Create: `.github/workflows/rust-ci.yml`
- Create: `rust/NOTICE`
- Create: `rust/README.md`

**Interfaces:**
- Produce: CI que valida fmt/clippy/test en Windows, macOS (arm + Intel) y Linux, y compila Tauri en los tres SO.

- [ ] **Step 1: Escribir el workflow**

Jobs: `test` (matriz `windows-latest`, `macos-15`, `macos-15-intel`, `ubuntu-22.04`; `cargo fmt --all --check`, `cargo clippy -p sqlt-core --all-targets -- -D warnings`, `cargo test -p sqlt-core` con `working-directory: rust`) y `tauri-check` (matriz `windows-latest`, `macos-15`, `ubuntu-22.04`; instala deps de WebKit en Linux, Node 22, `npm ci` usando el `package-lock.json` commiteado, build de Vite y `cargo check -p sqlt-desktop`).

- [ ] **Step 2: Escribir atribución y README**

`rust/NOTICE`: GPLv3, atribución a SQLTools (autores originales) y a SQLTools Mac; nota de que Instant Client no se redistribuye. `rust/README.md`: requisitos por SO, comandos de build/test, limitación de Instant Client.

- [ ] **Step 3: Verificar localmente lo que el CI va a correr**

Run (desde `rust/`): `cargo fmt --all --check; cargo clippy -p sqlt-core --all-targets -- -D warnings; cargo test -p sqlt-core`
Expected: todo verde.

- [ ] **Step 4: Commit y push**

```powershell
git add .github/workflows/rust-ci.yml rust/NOTICE rust/README.md
git commit -m "ci(rust): matriz Windows/macOS/Linux y atribución GPLv3"
git push
```

- [ ] **Step 5: Verificar el CI remoto**

Run: `gh run list --branch rust --limit 1`
Expected: workflow en curso o completado; si falla una plataforma, corregir antes de cerrar P0.

---

## Cierre de P0 (definición de done)

- [ ] `git log --oneline -4` muestra los commits de spec/plan, workspace/splitter, Tauri y CI.
- [ ] `cargo test -p sqlt-core` PASA (19 casos del original + `statement_at` + UTF-16 + solo comentarios + CRLF + identificador entrecomillado + `q'{ }'`).
- [ ] `cargo build -p sqlt-desktop` PASA en Windows (y CI en macOS/Linux).
- [ ] Workflow de GitHub Actions verde en las 4 plataformas.
- [ ] `SCOPE`: `rust/**`, `.github/workflows/rust-ci.yml`, `.gitignore`, `docs/superpowers/**`. Nada de `src/`, `ui/` ni `CMakeLists.txt`.
