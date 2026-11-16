# SQLTools Rust

Reescritura en Rust de SQLTools (cliente Oracle para PL/SQL/APEX), con UI web
(Tauri v2 + React) y objetivo multiplataforma: Windows x64, macOS Intel/Apple
Silicon y Linux x64.

Estado: **P0 (bootstrap)**. El port C++/Qt sigue intacto en la raíz del repo
(rama `main`); esta rama (`rust`) contiene la implementación nueva.

## Estructura

```
rust/
  crates/sqlt-core/   # lógica pura (partidor de sentencias, formato, binds...)
  desktop/            # shell Tauri v2
  web/                # frontend React + TypeScript + Vite
```

## Requisitos

- Rust stable (host MSVC en Windows; Xcode CLT en macOS; build-essential en Linux).
- Node.js 22+ y npm.
- Tauri v2: WebView2 en Windows, WebKitGTK en Linux.
- Oracle Instant Client **solo para la capa de base de datos** (fase P1). No se
  incluye en el repo por su licencia OTN.

## Comandos

```bash
# desde rust/
cargo fmt --all --check
cargo clippy -p sqlt-core --all-targets -- -D warnings
cargo test -p sqlt-core

# frontend
npm --prefix web install
npm --prefix web run build

# app de escritorio (compila el shell; el empaquetado llega en P5)
cargo build -p sqlt-desktop
```

## Licencia

GPLv3. Ver `LICENSE` en la raíz y `NOTICE` en esta carpeta.
