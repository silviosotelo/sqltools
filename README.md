# SQLTools

SQLTools is a cross-platform Oracle database client for PL/SQL and APEX developers.
This repository contains the current Rust/Tauri port under [`rust/`](rust/) and the
earlier Qt 6 macOS port.

- **Downloads:** [GitHub Releases](https://github.com/silviosotelo/sqltools/releases)
- **License:** GNU GPL v3 (see [`LICENSE`](LICENSE))

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by
[SignPath Foundation](https://signpath.org).

- Committers/authors and reviewers: [@silviosotelo](https://github.com/silviosotelo)
- Approvers: [@silviosotelo](https://github.com/silviosotelo)
- Process and artifact scope: [CODE_SIGNING_POLICY.md](CODE_SIGNING_POLICY.md)
- Privacy policy: [PRIVACY.md](PRIVACY.md)

## Current application (Rust + Tauri)

The Rust workspace under [`rust/`](rust/) builds the cross-platform desktop application
(Windows, macOS, Linux): Oracle connections with keyring storage, SQL editor, streaming
results, object explorer, DDL, SQL formatter, command palette, workspaces and i18n
(es/en). Build and test instructions are in [`rust/README.md`](rust/README.md).

---

## Legacy macOS Qt port (Spanish)

Port nativo para macOS de **[SQLTools](https://sqltools.net)**, el cliente Oracle para desarrolladores de PL/SQL y APEX (originalmente para Windows/MFC). Interfaz en Qt 6, conexión mediante Oracle Instant Client (OCI).

> Proyecto en desarrollo. La paridad con la versión de Windows es alta, pero no está todo probado contra todas las versiones de Oracle.

## Características
- Editor SQL/PL-SQL con resaltado, plegado de código, marcadores, autocompletado y objetos del esquema coloreados como en Windows.
- Explorador de objetos y lista de objetos (22 pestañas), Object Viewer con arrastre al editor, carga de DDL.
- Ejecución de scripts, resultados en rejilla, plan de ejecución, estadísticas, historial, variables bind.
- Gestor de conexiones con contraseñas en el Llavero de macOS, importación de las conexiones de SQLTools de Windows (sin contraseñas).
- Menús y atajos equivalentes a los de Windows, adaptados al Mac.

## Descargas para probar
| Su Mac | Carpeta | Descarga |
|---|---|---|
| Chip **Intel** | [`macIntel/`](macIntel/LEEME.md) | [⬇️ SQLTools-Mac-x86_64.zip](macIntel/SQLTools-Mac-x86_64.zip) |
| Chip **Apple (M1–M4)** | [`macM/`](macM/LEEME.md) | [⬇️ dos partes + instrucciones](macM/LEEME.md) |

## Documentación (en español)
- Manual de usuario: [`docs/manual/`](docs/manual/) (PDF y HTML en `docs/manual/salida/`)
- Menús y atajos: [`docs/MENUS_Y_ATAJOS.md`](docs/MENUS_Y_ATAJOS.md), [`docs/ATAJOS.md`](docs/ATAJOS.md)
- Comparación con Windows: [`docs/COMPARACION_WINDOWS_MAC.md`](docs/COMPARACION_WINDOWS_MAC.md)
- Diario de desarrollo y decisiones: [`PLAN.md`](PLAN.md)

## Compilar
Requisitos: macOS, Xcode Command Line Tools, CMake ≥ 3.21, Ninja, **Qt 6.8** y **Oracle Instant Client 19.x (Basic + SDK)**.

El Instant Client **no se incluye** en este repositorio (licencia de Oracle). Descárguelo de
<https://www.oracle.com/database/technologies/instant-client/macos-intel-x86-downloads.html> y descomprímalo en la raíz como `instantclient_19_16/` (o indique otra ruta con `-DORACLE_IC_DIR=...`).

```bash
cmake -S . -B build-release -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH=/ruta/a/Qt/6.8.3/macos
cmake --build build-release
open build-release/SQLToolsMac.app
```

Para generar el paquete distribuible: `bash tools/package_app.sh`. Para regenerar el manual: `bash tools/build_manual.sh`.

## Pruebas
`SQLT_SELFTEST=commands QT_QPA_PLATFORM=offscreen build-release/SQLToolsMac.app/Contents/MacOS/SQLToolsMac` ejecuta la autoprueba de comandos (no necesita base de datos).

## Licencia
GNU GPL v3, igual que SQLTools original (ver [`LICENSE`](LICENSE)). El código original de Windows está en `fuente_original/` como referencia; el copyright de SQLTools pertenece a sus autores.
