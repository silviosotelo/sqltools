# Plan: paridad funcional con PL/SQL Developer 17

Fecha: 2026-11-26. Estado: propuesto (planificado por el subagente `advisor`, con los datos de
pre-vuelo verificados por el agente principal).

Inventario de partida: `docs/parity/plsql-developer-17.md` (23 capacidades dentro de las 20 áreas del
proveedor: 4 completas, 9 parciales, 10 faltantes).

## Reglas del trabajo

- **Alcance B** (decisión del dueño): es nuestro producto. La paridad se construye con documentación
  pública y comportamiento observable. No se decompila ni se copian binarios, UI, iconos o textos de
  PL/SQL Developer. REA queda para interoperabilidad (formatos de archivo propios del usuario).
- Cada fase cierra con: instalador reconstruido e instalado, CDP contra la app instalada (conectar,
  explorar, ejecutar, Ctrl+F12), axe 0 en claro y oscuro, 1366×768 y 1024×700 sin desbordes,
  `cargo fmt/clippy -D warnings/check -p sqlt-desktop/test`, vitest, evidencia en el plan, commit y push.
- Los comandos nuevos mueven el digest de `rust/web/e2e/golden-commands.json`: se actualiza en el mismo
  commit.

## Datos de pre-vuelo (verificados hoy contra `anamnesis@desa02`)

| Dato | Resultado | Fuente |
|---|---|---|
| `DBMS_PROFILER` | **Presente y VALID** (SYS + sinónimo PUBLIC) | sonda por la conexión guardada en el keyring |
| `DBMS_DEBUG` | **Presente** (también `DBMS_DEBUG_JDWP`, `DBMS_TRACE`, `DBMS_APPLICATION_INFO`) | ídem |
| `v$sql` legible por la cuenta | **Sí** (12.768 filas) | ídem |
| `all_dependencies` | 211.357 filas → las consultas de dependencias necesitan filtro por objeto y límite | ídem |
| Binds de salida | La crate `oracle 0.6.3` los soporta (`Statement::bind_value`), pero **`execute_with` los descarta** y `Session` no expone `conn`/`statement`: hace falta un método nuevo en `sqlt-oracle` (ejecutar bloque + leer bind de salida) | `session.rs`, docs de la crate |
| 13 comandos sin implementar | `Edit.JumpNext`, `Edit.JumpPrevious`, `Test`, `Window.ArrangeIcons`, `Window.Cascade`, `Window.LastWindow`, `Window.NewWindow`, `Window.NextTab`, `Window.NextWindow`, `Window.PrevWindow`, `Window.PreviousTab`, `Window.TileHorizontally`, `Window.TileVertically` | `list_commands` por CDP |
| `compare.rs` | Es un diff de texto DDL para validar el port de MetaDictionary, **no** un comparador de esquemas | `crates/sqlt-meta/src/compare.rs` |

## Orden de fases

Ordenado por valor diario para un desarrollador PL/SQL dividido por costo y riesgo.

| Fase | Área | Depende de | Valor | Costo/riesgo |
|---|---|---|---|---|
| **P1** | #12 Object Browser completo: dependencias, enable/disable, recompile invalid | — | Alto (2.º panel más usado) | Bajo: solo diccionario y DDL |
| **P2** | #5 SQL Window editable + QBE sobre el resultado | — | Máximo (edición de datos es el uso diario) | Medio: ROWID, concurrencia optimista |
| **P3** | #1 Code Assistant (autocompletado por diccionario) | P1 | Alto diario | Medio-bajo: heurística léxica, sin parser |
| **P4** | #21 Compare User Objects + #11 To-Do Items | P1 | Medio | Medio (compare real) / bajo (scan de TODO) |
| **P5** | #16 PL/SQL Profiler por línea | — (necesita el método de binds de salida) | Alto para tuning | Medio |
| **P6** | #2 Debugger | **P5** | Máximo diferenciador | Alto (2-3× cualquier otra fase) |
| **P7** | #7 Reports + #13 File Browser + #23 invalid objects / import básico | P2 | Medio | Bajo |
| **P8** | #9 Projects + #6 Command Window UI | P2 | Medio | Medio |
| **P9** | #18 Formas DDL (tablas, sequences) + #15 Scheduler | P1 | Medio | Medio-alto |

**Por qué el Profiler antes que el Debugger:** los dos necesitan lo mismo —ejecutar una unidad PL/SQL con
binds IN/OUT y recoger resultados de la sesión—. El Profiler construye esa base con menos riesgo
(`DBMS_PROFILER` ya está instalado); el Debugger la reutiliza y agrega `DBMS_DEBUG` con sesión dedicada.
Al revés, esa infraestructura se construiría bajo la presión de la fase más riesgosa.

**Reports depende del grid editable** (renderiza resultados, exporta, recibe parámetros). **Compare y las
formas DDL dependen del árbol** (selección múltiple y menú contextual), que llega en P1.

## P1 — Object Browser completo (detalle)

| Item | Detalle |
|---|---|
| Alcance mínimo | (a) panel de dependencias por objeto: `ALL_DEPENDENCIES` en las dos direcciones, nivel 1 expandible, filtrado por objeto (la tabla tiene 211 k filas: nunca sin filtro); (b) habilitar/deshabilitar triggers y constraints con `ALTER`; (c) "Recompile invalid": lista de `all_objects` con `status='INVALID'`, `ALTER ... COMPILE REUSE SETTINGS` y errores de `ALL_ERRORS`; (d) todo en el menú contextual del árbol |
| Archivos | `crates/sqlt-app/src/catalog.rs` (+3 consultas), `commands.rs` (+6 comandos `Object.*`), `web/src/tree/`, `ObjectTree.tsx` (menú), `i18n.tsx` |
| Verificación Oracle | Crear un procedimiento roto → INVALID en el árbol → Recompile → error de `ALL_ERRORS` → corregir → VALID; deshabilitar un trigger de prueba y confirmar por `all_triggers`; con *Solo lectura* activo el cambio se rechaza con mensaje claro |
| Verificación CDP | `e2e/phase-p1.mjs` con el patrón de `phase-f5.mjs` (devuelve `Boolean`, nunca nodos), axe 0, dos resoluciones |

## P2 — SQL Window editable + QBE (detalle)

| Item | Detalle |
|---|---|
| Alcance mínimo | (a) solo consultas de **una tabla con ROWID** son editables; el resto queda de solo lectura con mensaje (sin parser de SQL); (b) ROWID oculto, edición en buffer y `UPDATE ... WHERE ROWID = :1 AND <col> = :viejo` (optimista; 0 filas afectadas = conflicto); (c) NULL explícito; (d) integrado al indicador de transacción y a commit/rollback/cancel de F5; (e) QBE como fila de filtros que re-ejecuta `SELECT * FROM (<sql>) WHERE <filtros>`, con el `ORDER BY` del usuario afuera del envoltorio |
| Archivos | `sqlt-oracle/session.rs` (`execute_with` con binds ya existe), `sqlt-app/src/grid.rs` (nuevo), `commands.rs` (`Grid.SaveChanges`, `Grid.CancelEdits`, `Grid.ToggleQBE`), `web/src/results.ts`, `EditableGrid.tsx` |
| Verificación Oracle | Editar → guardar → SELECT confirma; editar en paralelo desde SQL*Plus → conflicto detectado; rollback revierte; vista deshabilitada; read-only bloquea |
| Verificación CDP | Doble clic, escribir, guardar; snapshot del grid y del indicador; axe claro/oscuro |

## P3 — Code Assistant (detalle)

| Item | Detalle |
|---|---|
| Alcance mínimo | completado de CodeMirror alimentado por objetos del owner (`all_objects`), columnas de las tablas ya referenciadas (consulta existente) y alias detectados por tokenización léxica (sin parser; `ponytail:` con la ruta de mejora). Disparo al escribir y con `Ctrl+Space`; tope de 100 resultados; refresco con `Session.RefreshObjectCache` |
| Archivos | `web/src/Editor.tsx` (completion source), `catalog.rs` (+1 consulta con caché), `commands.rs` (`Edit.Autocomplete` ya registrado) |
| Verificación | En SQL window: prefijo → popup con tablas; `alias.` → columnas; teclado flechas + Enter; axe y `keyboard-walk` en verde |

## Riesgos

| Riesgo | Mitigación |
|---|---|
| `DBMS_DEBUG` bloquea la sesión durante el debug y deja inútil el Cancel de F5 | sesión OCI dedicada, timeout obligatorio, `DEBUG_OFF` garantizado; verificar antes de prometer la función |
| Binds de salida ausentes en `Session` | agregar el método en `sqlt-oracle` en P5 (la crate lo soporta); no cambiar `execute_with` |
| QBE con `ORDER BY`/`ROWNUM` dentro del subselect | copiar el `ORDER BY` afuera; caso de prueba específico en 12c |
| Grid editable prometido de más | límite explícito a tabla única con ROWID; el mensaje de solo lectura es parte del feature |
| Digest de comandos | actualizarlo en el mismo commit |
| Dependencias nuevas | ninguna en P1–P9: CodeMirror, Rust std y la crate `oracle` |

## Anti-patrones (descartados)

1. **Parser de PL/SQL completo** para refactor, QBE o compare semántico: solo tokenización local.
2. **Formas DDL genéricas** con UI por metadatos: empezar por tablas y sequences dedicadas, siempre con
   vista previa del DDL y nunca ejecutar directo.
3. **UI completa de Data Pump** para import/export: el CLI ya cubre ese caso.
4. **Reimplementar los comandos MDI**: no tienen sentido en una app de pestañas.

## Fuera de alcance (recomendado)

| Área | Motivo |
|---|---|
| #8 Diagrams | editor de grafos SVG es un proyecto aparte, de valor puntual |
| #20 Query Builder | mismo costo; el QBE del grid (P2) cubre la necesidad diaria |
| #3 AI Assistant | decisión de producto separada (claves, proveedores); si se hace, provider-agnostic y al final |
| #10 Test Manager completo | el script runner existente cubre el 80 %; un "correr set de scripts y reportar" entra barato en P7 |
| Comandos MDI (`Window.*`) | propuesta: **retirarlos del registry** (no dejarlos como no-ops), actualizando el golden, con decisión del dueño |
| `File.Print*` | se deja el stub actual; baja frecuencia |
| Import ODBC / texto | nicho de migración, no de desarrollo diario |

## Decisiones abiertas del dueño

1. ¿Se arranca por P1 (árbol) o por P2 (grid editable)? Las dos son correctas; P2 toca datos reales.
2. ¿Se retiran los 13 comandos sin implementar del registry o se dejan listados?
3. ¿El AI Assistant queda fuera, o se planifica al final como proveedor agnóstico?
