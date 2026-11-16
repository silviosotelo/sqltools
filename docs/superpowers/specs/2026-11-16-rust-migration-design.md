# Migración de SQLTools Mac a Rust — diseño

Fecha: 2026-11-16
Estado: aprobado en conversación (enfoque A, Tauri + CodeMirror, paridad por fases)
Licencia: GPLv3 (deriva de SQLTools y de SQLTools Mac)

## 1. Objetivo

Reescribir SQLTools (cliente Oracle para desarrolladores PL/SQL/APEX) en Rust con
UI web (Tauri v2 + React + CodeMirror 6), multiplataforma real
(Windows x64, macOS Intel/Apple Silicon, Linux x64), con paridad funcional
progresiva respecto del port C++/Qt actual y mejoras de rendimiento,
concurrencia, extensibilidad, seguridad y distribución.

Éxito = v1 empaquetada en los tres SO que cubre los flujos diarios (conectar,
editar, ejecutar, ver/exportar resultados, explorar objetos, cargar DDL) con
verificación ejecutada contra una base Oracle real, no solo compilación.

## 2. Contexto y restricciones

- Base actual: `src/` + `ui/` (~29k líneas C++17/Qt 6) y `fuente_original/`
  (~245k líneas del SQLTools de Windows, solo referencia). 226/235 comandos
  implementados; 9 suites de pruebas y 146 comprobaciones de autotest.
- Comportamiento ya validado que NO se puede degradar: 25/25 tipos de datos,
  12/12 casos de binds, formato de fechas contra `TO_CHAR` (260 000 valores ×
  13 formatos, 0 diferencias), cancelación en ~2 s con conexión reutilizable,
  resolución de wallets/TLS, DDL idéntico al de Windows.
- El cuello de rendimiento medido es red/prefetch, no CPU. Rust no cambia eso;
  sí mejora arranque, memoria, concurrencia y distribución.
- Oracle Instant Client es requisito de runtime y NO se redistribuye en el
  repositorio (licencia OTN). El driver lo carga dinámicamente.
- El C++ actual no se modifica: la rama `main` conserva el port; el trabajo
  nuevo vive en la rama `rust` bajo `rust/`.
- Este equipo de desarrollo es Windows y no tiene toolchain Rust, CMake ni
  Instant Client instalados: P0 incluye instalarlos.

## 3. Decisiones de arquitectura

### 3.1 Acceso a Oracle

Se usa el crate `oracle` (Rust async sobre ODPI-C, runtime tokio), que carga el
Instant Client en tiempo de ejecución. Se reserva FFI directo a OCI para casos
puntuales que el crate no cubra (p. ej. `OCIBreak` si no hay API equivalente).

Rechazados: port directo de `src/OCI8` a `unsafe` Rust (36k líneas de manejo de
handles, máxima fidelidad pero coste y riesgo desproporcionados); bindings
ODPI-C crudos (más código propio sin beneficio claro).

Paridad se demuestra con las mismas suites y datasets del port actual, más
golden files extraídos de las pruebas C++ existentes.

### 3.2 Estructura del repositorio (rama `rust`)

```
rust/
  Cargo.toml              # workspace
  crates/
    sqlt-core/            # dominio puro, sin IO: split, binds, formato, fold,
                          # outline, contexto de autocompletado, clasificación
                          # de sentencias, historial, ajustes, exportadores,
                          # registro de comandos
    sqlt-oracle/          # driver: sesiones, streaming, cancelación,
                          # TNS/wallet, modo solo lectura, tipos y fechas
    sqlt-meta/            # generación DDL (fase P4)
    sqlt-app/             # casos de uso: connect, execute, export, browse...
    sqlt-cli/             # harness headless (integración, benchmarks, diag)
  desktop/                # app Tauri v2 (comandos, estado, canales, keyring)
  web/                    # React + TS + Vite + CodeMirror 6
  tests/                  # integración Oracle (gated por env)
  fixtures/               # golden files y datos de prueba
.github/workflows/ci.yml
```

El C++ existente queda intacto en la raíz del repo.

### 3.3 Capas y flujo

```
React UI ──invoke/Channel──► Tauri commands ──► sqlt-app (casos de uso)
                                                   │
                                          ┌────────┴────────┐
                                    sqlt-core        sqlt-oracle
                                  (puro, testeable)  (tokio + ODPI-C)
```

- Cada conexión es una tarea tokio con cola propia. Hay sesión principal y
  sesión de fondo (listas del explorador, precarga) como en el port actual.
- Resultados en streaming por lotes (prefetch configurable, 1k–20k filas) con
  backpressure; la grilla virtualiza en el frontend. Memoria acotada por
  configuración (máximo de filas y de caracteres por celda).
- Exportación (CSV/TSV/JSON/XML/INSERT/HTML/texto) se escribe en streaming
  desde Rust; no se serializa el resultado completo por IPC.
- Cancelar = token de cancelación + break del driver; la sesión queda usable.
- La inteligencia de texto SQL vive en `sqlt-core` y se expone como
  `analyze(sql)`: sentencias, folds, outline, contexto y errores. El editor solo
  aplica decoraciones y pide completions. Una implementación, testeable sin UI.
- Números Oracle se transportan como string canónica (38 dígitos, sin pérdida
  de precisión); la metadata indica alineación y tipo de columna.

### 3.4 UI

- Tauri v2; React 19 + TypeScript + Vite.
- CodeMirror 6 con dialecto Oracle: resaltado, plegado, marcadores, búsqueda,
  selección rectangular y multi-cursor nativos; autocompletado asíncrono contra
  el backend con debounce y caché.
- Grilla: TanStack Table + virtualización; paneles Result/Output/Errors/
  Statistics/SQL Plan/History/Binds.
- El diseño visual (layout, estilos, tema, jerarquía) NO se define acá: lo
  especifica `@designer`. Este documento fija estructura y comportamiento.

### 3.5 Seguridad y secretos

- Contraseñas solo en el keyring nativo (`keyring` crate: Credential Manager,
  Keychain, Secret Service). Nunca en JSON, logs ni diagnóstico.
- Resolución de TNS/wallet portada a Rust con la lógica actual (ruta del alias,
  `sqlnet.ora`, carpeta temporal corregida sin tocar archivos del usuario).
- Modo solo lectura aplicado en `sqlt-app` antes de ejecutar, con clasificación
  de sentencia; pruebas propias.
- Redacción de secretos en logs y en el diagnóstico de "Acerca de".

## 4. Fases y criterios de aceptación

| Fase | Alcance | Done (verificado ejecutando) |
|---|---|---|
| P0 | Fork, rama `rust`, toolchain, workspace, CI matriz 3 SO, esqueleto Tauri, licencia/atribución | CI verde (fmt, clippy, test, build) en Windows/macOS/Linux; `cargo tauri build` produce artefacto |
| P1 | `sqlt-oracle`: conectar (alias/directo/wallet), ejecutar, tipos, binds, fechas, LOBs, cancelar, NLS | Suite integración contra Oracle real: 25/25 tipos, 12/12 binds, golden de fechas 260k×13 sin diferencias, cancelación ≤3 s y reutilizable, wallet/TLS; benchmark de filas/s documentado |
| P2 | App v1: conexiones+keyring, editor (resaltado/plegado/marcadores/autocompletado/buscar), ejecución sentencia/script/selección, grilla streaming, cancelar, Output, History, Binds, Plan, Statistics, exportar | E2E Playwright de los flujos + regresión Oracle; checklist de paridad P2 completo |
| P3 | Explorador de objetos, Object Viewer, menús contextuales, Load DDL vía DBMS_METADATA | Tests de integración Oracle + checklist de paridad P3 |
| P4 | `sqlt-meta` (DDL idéntico a Windows), Format SQL, workspaces/ajustes, 226 comandos, i18n es/en | Golden DDL vs muestra de Windows; 20 casos de formato; comandos <-> `menu_spec.json` |
| P5 | Instaladores MSI/DMG/AppImage, firma/notarización, updater, manual, migración de preferencias del port | Smoke test desde el paquete instalado contra Oracle en cada SO |

Fuera de v1 (fase posterior): multi-motor (PostgreSQL/MySQL/SQL Server),
asistente IA, integración Git, comparación de esquemas, editor de datos.

## 5. Estrategia de verificación

1. **Golden files primero**: extraer casos de `tests/*.cpp` (split 19, contexto
   26, formato 20, fold 15, wallet 17, export 11, viewer, meta) a fixtures y
   probar el port Rust contra ellos. Es la red de seguridad barata.
2. **Integración Oracle** con las mismas variables del port (`SQLT_USER`,
   `SQLT_PASS`, `SQLT_ALIAS`, `SQLT_TNS_ADMIN`); se salta si faltan. Sin
   credenciales en el repo.
3. **E2E**: Playwright contra el frontend con IPC simulado para flujos de UI;
   `tauri-driver` en Windows/Linux para los flujos reales; macOS con smoke
   manual documentado (limitación conocida de WebDriver en macOS).
4. **Benchmarks**: `criterion` para funciones puras; `sqlt-cli bench` contra
   Oracle comparando filas/s y memoria contra el port C++.
5. **CI**: fmt + clippy `-D warnings` + tests + build Tauri en matriz. La
   integración Oracle corre local/self-hosted, no en GitHub público.

## 6. Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| El crate `oracle` no cubre algún caso del port (LOBs, NLS, break) | Suite de paridad P1 como puerta; FFI puntual a OCI si hace falta |
| Paridad del editor (columna, folds, bookmarks) menor que Qt | CodeMirror cubre nativo; completar con extensiones y golden tests de `analyze` |
| Port del DDL MetaDictionary (10.6k líneas) | Fase P4 aislada, golden contra la salida de Windows; DBMS_METADATA como fallback en P3 |
| Firma/notarización y updater requieren certificados/keys | P5 con checklist; no bloquea P0–P4 |
| Instant Client no redistribuible | Detección en runtime y documentación de instalación; nunca empaquetado por defecto |
| WebDriver en macOS | Smoke manual + pruebas de UI desacopladas del shell |
| Deriva de alcance por "mejorar y escalar" | Fases con done explícito; lo nuevo fuera de v1 queda listado |

## 7. Atribución y cumplimiento

- GPLv3 para todo el código derivado; `LICENSE` se conserva.
- `NOTICE` con atribución a SQLTools (autores originales) y a SQLTools Mac.
- El binario distribuido debe ofrecer el código fuente correspondiente.
- Oracle Instant Client: licencia OTN aparte; se documenta su instalación y no
  se incluye en el repo ni en los paquetes por defecto.

## 8. Preguntas abiertas (no bloqueantes)

- Nombre comercial del fork y del paquete (`com.sqltools.rust` propuesto).
- Certificados de firma y host de actualizaciones (P5).
- Versión mínima de Oracle soportada además de 12.1/19c (se prueba con las
  accesibles).
