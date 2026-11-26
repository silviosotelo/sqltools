# Goal: paridad funcional con PL/SQL Developer 17

Spec/inventario: `docs/parity/plsql-developer-17.md`.
Plan: `docs/superpowers/plans/2026-11-26-parity-plsql-developer.md`.

## Objetivo

Llevar SQLTools (Rust + Tauri + React) a paridad funcional con PL/SQL Developer 17, ejecutando las fases
**P1 a P10 en orden estricto**: una fase no empieza hasta que la anterior esté cerrada con la compuerta de
este archivo cumplida, con salida real. No se pide confirmación entre fases.

| Fase | Alcance |
|---|---|
| P1 | Object Browser completo: dependencias, enable/disable de triggers y constraints, recompile invalid, menú contextual. Incluye la auditoría de los 13 comandos sin implementar contra los menús de PL/SQL Developer |
| P2 | SQL Window editable (tabla única con ROWID, concurrencia optimista) + QBE sobre el resultado |
| P3 | Code Assistant: autocompletado por diccionario y alias (sin parser de PL/SQL) |
| P4 | Compare User Objects (comparador real de esquemas, no diff de texto) + To-Do Items |
| P5 | PL/SQL Profiler por línea, sobre `DBMS_PROFILER` (ya instalado en desa02) |
| P6 | Debugger con `DBMS_DEBUG` en sesión dedicada, con timeout y salida garantizada |
| P7 | Reports, File Browser e invalid objects / import básico |
| P8 | Projects (archivos + objetos + notas) y Command Window UI |
| P9 | Formas DDL para tablas y sequences, y DBMS Scheduler |
| P10 | AI Assistant, agnóstico de proveedor, con la clave en el keyring |

## Alcance (no negociable)

- Es **nuestro producto**: la paridad se construye con documentación pública y comportamiento observable.
  **No** se decompila PL/SQL Developer ni se copian sus binarios, UI, iconos o textos.
- REA se usa solo para interoperabilidad con archivos del usuario y para analizar artefactos propios.
- Todo texto nuevo por `i18n.tsx` (es/en). Código, identificadores, tests y errores en inglés.
- La app sigue usable y verificada después de cada fase.
- Se omite lo que no tienen sentido en una app de pestañas (MDI) y lo que el dueño dejó fuera:
  Diagrams, Query Builder, Test Manager completo, impresión, import ODBC/texto.
- Los comandos sin implementar se retiran del registry **solo si PL/SQL Developer no los implementa**,
  con la evidencia de la auditoría anotada; el digest de `golden-commands.json` se actualiza en el mismo
  commit.
- Secretos: las claves de AI (P10) van al keyring del sistema, nunca al JSON de configuración ni a un
  archivo de texto.

## Cierre de CADA fase (todo, con salida real; sin esto no se pasa a la siguiente)

1. `cargo fmt --all -- --check`, `cargo clippy --workspace --exclude sqlt-desktop --all-targets -- -D warnings`,
   `cargo check -p sqlt-desktop`, `cargo test --workspace --exclude sqlt-desktop` y
   `npm --prefix rust/web run build` en verde; `npm --prefix rust/web test` (vitest) en verde.
2. Instalador reconstruido e instalado; **CDP en la app instalada** recorre conectar, explorar, ejecutar y
   cargar DDL (`Ctrl+F12`) sin regresiones (`flow.mjs` idéntico a su golden).
3. `axe-cdp.mjs`: 0 violaciones en cada pantalla nueva, tema claro y oscuro.
4. 1366×768, 1024×700 y 1024×640 sin desbordes ni controles cortados.
5. Verificación contra Oracle real (`anamnesis@desa02`) con el caso propio de la fase, usando la conexión
   guardada (keyring) o el patrón de `crates/sqlt-app/examples/live_check.rs`; nunca se escribe una
   contraseña en archivos.
6. Evidencia anotada en el plan (comandos, salida, binario y commit), commit y push a `rust`, `MEM-SAVE`.

Un problema real fuera de alcance se reporta, no se arregla. Si aparece algo que solo el dueño puede
resolver (decisión, credencial, paso manual), se llama a `opencode_loop_goal_blocked` y se detiene.

## Reglas de trabajo

- Método del dueño (AGENTS.md) en cada fase: plan antes de codificar, consulta al `advisor` o `codex exec`
  antes de comprometer el enfoque y antes de cerrar, verificación ejecutada, `INTENT/TWINS/SCOPE/MEM-SAVE`.
- Si el `advisor` y `codex` no están disponibles por límite de uso: declararlo y no simular la aprobación.
- El subagente `reviewer` se usa para la revisión independiente de cierre si `codex` no está.
- Cada fase propone su plan detallado en `docs/superpowers/plans/2026-11-26-parity-plsql-developer.md`
  antes de escribir código, y lo deja actualizado al cerrar.

## Límites del loop

`--max-runs 40 --max-runtime 12h --max-failures 3 --safe --stop-file .stop`

## Cierre del goal

Solo con evidencia de las diez fases: capturas de la app real en `docs/screenshots/parity-*.png`,
`rust/web/DESIGN.md` actualizado si la UI cambió, inventario de paridad actualizado con los estados finales,
y `opencode_loop_goal_complete`.
