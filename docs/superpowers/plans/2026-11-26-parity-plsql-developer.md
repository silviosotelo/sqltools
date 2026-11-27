# Plan: paridad funcional con PL/SQL Developer 17

Fecha: 2026-11-26. Estado: aprobado. Planificado por el subagente `advisor`; los datos de pre-vuelo los verificó el agente principal; las decisiones abiertas las resolvió el dueño: secuencia estricta (sin cerrar una fase no empieza la siguiente), retirar comandos solo si PL/SQL Developer no los implementa, y planificar el AI Assistant (P10).
pre-vuelo verificados por el agente principal).
Goal: `docs/superpowers/goals/2026-11-26-parity-plsql-developer-goal.md`.

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
| **P10** | #3 AI Assistant agnóstico de proveedor | P3 (usa el mismo contexto de editor) | Alto para el usuario final | Medio: integración, claves y costo |

**Secuencia estricta:** una fase no empieza hasta que la anterior cierre la compuerta del goal. El orden de la tabla es el orden de ejecución.

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
| Auditoría de los 13 comandos | Antes de tocar el registry: observar los menús de PL/SQL Developer 17 en pantalla (comportamiento observable, sin extraer cadenas del binario) y anotar, uno por uno, si existe: `Edit.JumpNext`, `Edit.JumpPrevious`, `Test` y los once `Window.*`. Se retiran del registry solo los ausentes; los presentes quedan listados y justificados |

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
| #10 Test Manager completo | el script runner existente cubre el 80 %; un "correr set de scripts y reportar" entra barato en P7 |
| Comandos MDI (`Window.*`) | se auditan en P1 contra los menús de PL/SQL Developer; se retiran solo los que no existan allí |
| `File.Print*` | se deja el stub actual; baja frecuencia |
| Import ODBC / texto | nicho de migración, no de desarrollo diario |

## P10 — AI Assistant (detalle)

| Item | Detalle |
|---|---|
| Alcance mínimo | Cliente agnóstico de proveedor (endpoint compatible OpenAI por configuración), funciones predefinidas sobre la selección o el documento (crear, completar, corregir, explicar, comentar, revisar, optimizar) y funciones propias del usuario con su prompt. La clave se guarda en el keyring del sistema, nunca en el JSON ni en un archivo de texto. Sin telemetría. Cancelable y con timeout |
| Archivos | `sqlt-app/src/ai.rs` (nuevo: cliente, funciones, validación), comando Tauri `ai_run`, `web/src/components/AiPanel.tsx`, `commands.rs` (`Tools.AiRun`), `i18n.tsx` |
| Verificación | Tests con un servidor HTTP local que simula el proveedor (no gasta cuota ni necesita clave); una llamada real solo si el dueño aporta una clave, y si no se declara NO VERIFICADO |
| Riesgo | Costo y privacidad del código enviado: la UI advierte que el texto sale de la máquina y nunca se envía nada sin acción explícita |

## Decisiones tomadas por el dueño

1. **Orden:** P1 a P10 en secuencia estricta; no se avanza sin cerrar la fase anterior.
2. **Comandos sin implementar:** se retiran del registry solo los que PL/SQL Developer no implemente, con la auditoría de P1 como evidencia. Los que existan en PL/SQL Developer se quedan listados.
3. **AI Assistant:** se planifica como P10, agnóstico de proveedor.

## Evidencia por fase

### P1 — cerrada (2026-11-26)

**Qué cambió:** `crates/sqlt-app/src/browser.rs` (nuevo módulo): dependencias por objeto en las dos
direcciones sobre `ALL_DEPENDENCIES` (siempre filtradas: la tabla tiene 211 k filas), objetos inválidos,
`ALTER ... COMPILE REUSE SETTINGS` con errores de `ALL_ERRORS`/`USER_ERRORS`, recompilación de todos los
inválidos, `ALTER TRIGGER ... ENABLE|DISABLE` y `ALTER TABLE ... ENABLE|DISABLE CONSTRAINT`. Los
identificadores se citan y se rechazan si traen comillas; el tipo de objeto se valida contra una lista
blanca antes de llegar al `ALTER` (2 tests unitarios). Comandos Tauri nuevos: `list_dependencies`,
`list_invalid_objects`, `compile_object`, `compile_invalid_objects`, `list_compile_errors`, `toggle_trigger`,
`toggle_constraint`. UI: pestaña **Dependencias** en el panel de objeto (ambas direcciones), botón
**Recompilar** en la cabecera, botón **Recompilar objetos inválidos** con el conteo en el panel de objetos,
acciones habilitar/deshabilitar por fila en Restricciones y Triggers, y **Dependencias / Recompilar /
Habilitar-Deshabilitar** en el menú contextual del árbol.

**Auditoría de los 13 comandos sin implementar (tarea de P1):** intenté leer los menús de PL/SQL Developer 17
por su propia interfaz de accesibilidad (UI Automation) y por el `HMENU` de la ventana. Resultado: la app es
Delphi con ribbon de DevExpress y **no publica** ni árbol UIA ni menú clásico (0 controles expuestos), así que
no hay evidencia observable de qué comandos implementa. Decisión aplicada: **no se retira ningún comando**
(el dueño autorizó retirar solo lo que PSD no implemente, y sin evidencia no se toca el registry). Los 13
siguen listados; el digest de `golden-commands.json` queda igual.

**Verificación contra Oracle (anamnesis@desa02, conexión del keyring):** sonda temporal
`examples/p1_probe.rs` (se borró después; creaba y eliminaba sus propios objetos):

```
invalid after broken create: PKG_APP_VENTAS_ESCRITURA=PACKAGE BODY, CARGAR_PLAN_WEB=PROCEDURE, ..., SQLT_P1_PROBE=PROCEDURE, ...
compile errors: 2
after recompile of the broken unit: 2 error(s)
status after fixing: VALID
AS_PDF3_MOD package body references 17 object(s)
objects depending on A11: 2
trigger after disable: DISABLED / after enable: ENABLED
constraint after disable: DISABLED / after enable: ENABLED
compiling a TABLE refused: true
leftovers: table=0 procedure=0
P1 PROBE OK
```

**Evidencia en la app instalada** (19:5x–20:0x, firmada autofirmada):

| Comando | Resultado |
|---|---|
| `node e2e/phase-p1.mjs` | `P1 CHECKS OK (15)`: pestaña Dependencias con A11 → `ANAMNESIS.FECHA_REP_AUX2` y `PUBLIC.A11` como dependientes; menú contextual con Dependencias/Recompilar; recompilar `AS_PDF3_MOD` → "Compilado sin errores"; botón de inválidos con el conteo (3) → "Recompilados 3; quedan 3 inválidos"; acciones por fila en las restricciones de `ARCHIVO_AUDITORIA_OPERATIVA` (9) y en los triggers de `NOT_CRED_PREST_REPAGA` (4) |
| `axe-cdp.mjs` | 0 violaciones, 8 vistas × 2 temas |
| `phase-f1` / `phase-f2` / `phase-f5` / `tree` / `env` / `keyboard-walk` | 19 / 3 / 39 / 11 / 12 / 26 paradas |
| `commands-check --compare` | OK, mismo digest (sin cambios de registry) |
| `flow --compare golden-f3.json` | IDENTICAL |
| capturas 1366×768, 1024×700 y 1024×640, claro y oscuro | `docs/screenshots/parity-p1-dependencies-*.png`, sin desbordes y `clipped: 0` |
| `cargo fmt` / `clippy -D warnings` / `check -p sqlt-desktop` / `test` | 0 / 0 / 0 / 0 (19 ok; +2 tests nuevos) |
| `npm run build` / `npm test` | 0 / **81 passed** |

**Hallazgo fuera de alcance (se reporta):** el esquema `ANAMNESIS` ya tenía objetos inválidos antes de esta
fase (`PKG_APP_VENTAS_ESCRITURA`, `CARGAR_PLAN_WEB`, `CARGAR_PLAN_WEB2`, `MIGRA_ADHERENTE`,
`RESCISION_CONTRATO_JNTRG`); la recompilación los intenta y deja 3 todavía inválidos. No es un defecto de la
app: son errores de esos objetos en la base.

### P2 — cerrada (2026-11-26)

**Decisión de diseño (sin parser de SQL):** el grid es editable solo cuando la última sentencia es un
`SELECT` de **una tabla** con lista de columnas simple, sin `join`, `union`, `group by`, `distinct`, agregados,
alias de tabla ni variables de bind. Lo decide `web/src/grid/editable.ts` con un escaneo léxico (paréntesis y
comillas respetados) y 8 tests de vitest. En ese caso la app ejecuta el mismo `SELECT` con
`t.ROWID as "__sqlt_rowid"` al final, oculta esa columna y guarda el ROWID alineado con cada fila.

**Edición:** doble clic en una celda abre el editor con el valor actual; `Aplicar` (o `NULL`) deja el cambio en
un buffer. `Guardar cambios` aplica uno por uno
`update <tabla> set <col> = :1 where rowid = :2 and (<col> = :3 or (<col> is null and :3 is null))`:
`1` fila afectada = guardado, `0` = conflicto (la fila cambió desde que se leyó) y se informa la fila, sin
pisar nada. `Descartar` limpia el buffer. Los identificadores se citan y se rechazan si traen comillas; la tabla
se deja sin citar cuando es un identificador simple para que Oracle pliegue mayúsculas (`a11` encuentra `A11`).
2 tests de Rust sobre la sentencia.

**QBE:** una condición `WHERE` en la barra del panel envuelve la consulta del usuario
(`select * from (<sql>) where <condición>`) y deja el `ORDER BY` afuera (test de vitest incluido).

**Regresiones propias detectadas y corregidas en la fase:**
1. Las **vistas** (`user_constraints`, `all_objects`) no tienen ROWID: el `SELECT` reescrito fallaba. Ahora, si
   el reescrito falla, la app reintenta con la sentencia del usuario y deja el grid de solo lectura.
2. El **historial** guardaba el SQL reescrito. El comando `execute` recibe un `historySql` opcional y el registro
   conserva el texto del usuario (`phase-f5` vuelve a verificar el historial).
3. El mensaje de guardado lo pisaba el refresco; ahora se informa después de refrescar.

**Verificación contra Oracle (conexión del keyring), sonda temporal y autolimpiante:**

```
rowids: AADxFFABQAAIAGNAAA AADxFFABQAAIAGNAAB
update with the value as read -> true
update with a stale value -> false (false = conflict)
set NULL -> true
set a value where it was NULL -> true
edit the row that was NULL -> true
unsafe table refused: true
leftovers: 0
P2 PROBE OK
```

**Evidencia en la app instalada** (21:23, firmada autofirmada):

| Comando | Resultado |
|---|---|
| `node e2e/phase-p2.mjs` | `P2 CHECKS OK (11)`: grid editable con ROWID oculto, editor abre con el valor, cambio en buffer (no se envía), guardado "1 filas guardadas", valor visible en el grid, celda a NULL, QBE filtra a 1 fila, un join queda de solo lectura |
| `phase-p1` / `phase-f5` | 15 / 39 (el historial conserva el SQL del usuario) |
| `axe-cdp.mjs` | 0 violaciones, 8 vistas × 2 temas |
| `phase-f1` / `phase-f2` / `tree` / `env` / `keyboard-walk` | 19 / 3 / 11 / 12 / 27 paradas |
| `commands-check` / `flow` | mismo digest / IDENTICAL |
| capturas 1366×768, 1024×700 y 1024×640, claro y oscuro | `docs/screenshots/parity-p2-editable-grid-*.png`, `clipped: 0` |
| `cargo fmt` / `clippy -D warnings` / `check -p sqlt-desktop` / `test` | 0 / 0 / 0 / 0 (19 ok) |
| `npm run build` / `npm test` | 0 / **89 passed** (8 nuevos del rewriter) |

**Límites declarados:** solo tabla única (una vista, un join o un agregado quedan de solo lectura, con el
mensaje de la propia app); no hay edición por lotes ni de varias celdas a la vez; el QBE es un `WHERE` único,
no una fila de filtros por columna.

### P3 — cerrada (2026-11-26)

**Qué cambió:** el autocompletado deja de ser una regex y pasa a un escaneo léxico testeado
(`web/src/editor/completion.ts`): tokeniza respetando comillas, comentarios y paréntesis; `tablesInScope`
reconoce `from`/`join`/`update`/`into`, alias con o sin `AS`, `owner.tabla` y listas con coma; `statementAt`
limita al enunciado del cursor; `tableForQualifier` resuelve alias antes que nombre; `limitSuggestions`
quita duplicados y corta en 100. **9 tests de vitest.**

En el editor, `alias.` ofrece las columnas de esa tabla y, sin calificador, primero las columnas de las tablas
ya nombradas (con `boost` para que CodeMirror no las mande al final), después los objetos del dueño con su
tipo, las tablas y las palabras clave. El catálogo sale de `catalog::suggest_objects` (nuevo): los nueve tipos
del árbol, ordenados por nombre y tope de 5000, recargado al conectar, al cambiar de esquema y con
`Session.RefreshObjectCache`.

**Dos defectos propios corregidos durante la fase:**
1. El nombre que llega desde el editor está en minúsculas (`a11.`) y `user_tab_columns` compara exacto: no
   aparecía ninguna columna. Ahora se normaliza a mayúsculas cada parte no citada antes de consultar.
2. Las columnas se perdían detrás del tope de 100 porque CodeMirror ordena por puntaje; se marcan con
   `boost: 1` para que el contexto gane al catálogo.

**Verificación contra Oracle (conexión del keyring), sonda temporal:**

```
user scope: 5000 objects, types: ["FUNCTION", "PACKAGE", "PROCEDURE", "SEQUENCE", "TABLE", "TRIGGER", "TYPE", "VIEW"]
capped at 50: 50
owner scope: 5000
P3 PROBE OK
```

**Evidencia en la app instalada** (22:06, firmada autofirmada):

| Comando | Resultado |
|---|---|
| `node e2e/phase-p3.mjs` | `P3 CHECKS OK (4)`: el comando devuelve >1000 objetos con ≥6 tipos; `t` → `FECHA`, `TIPO_CTO`, `TIPO_EMP`; sin calificador aparecen las columnas de `A11`; un calificador desconocido no inventa nada |
| `phase-p1` / `phase-p2` / `phase-f5` | 15 / 11 / 39 |
| `axe-cdp.mjs` | 0 violaciones, 8 vistas × 2 temas |
| `phase-f1` / `phase-f2` / `tree` / `env` / `keyboard-walk` | 19 / 3 / 11 / 12 / 27 paradas |
| `commands-check` / `flow` | mismo digest / IDENTICAL |
| capturas con el popup abierto, 1366×768, 1024×700 y 1024×640, claro y oscuro | `docs/screenshots/parity-p3-completion-*.png`, `popup: true`, `clipped: false`, sin scroll de página |
| `cargo fmt` / `clippy -D warnings` / `check -p sqlt-desktop` / `test` | 0 / 0 / 0 / 0 (19 ok) |
| `npm run build` / `npm test` | 0 / **98 passed** (9 nuevos) |

**Límites declarados:** el catálogo se corta en 5000 nombres (un esquema mayor sugiere menos, nunca rompe);
no hay sugerencias de columnas de tablas en subconsultas anidadas ni de paquetes (`pkg.proc`).

### P4 — cerrada (2026-11-26)

**Comparador real (no el diff de texto de `sqlt-meta`):** `crates/sqlt-app/src/compare.rs` toma un
**snapshot** de las fuentes del esquema (`ALL_SOURCE`/`USER_SOURCE`, una sola consulta por tipo+nombre, sin
DDL por objeto) y lo compara con la base actual. Clasifica cada objeto en **idéntico / distinto / faltante /
sobrante**, informa la primera línea y cuántas difieren, y genera el **script** que llevaría la base al
snapshot (solo lo que difiere o falta, con el encabezado del objeto). El snapshot se guarda como JSON estable
(clave `TIPO:NOMBRE`, porque `PACKAGE` y `PACKAGE BODY` comparten nombre). 4 tests unitarios.

**To-Do Items:** `scan_todos` recorre `ALL_SOURCE` buscando marcas y las filtra en Rust: la marca debe estar en
mayúsculas, ser palabra completa y **abrir el comentario** (entre el último `--`/`/*` y la marca solo se admite
puntuación). Sin ese filtro la base devolvía **443** coincidencias, casi todas la palabra española "todos";
con el filtro quedan **16 notas reales** (`-- TODO: Guardar BLOB en storage…`).

**UI:** quinta vista lateral **To-Do** (lista con marca, tipo, objeto y línea; al hacer clic abre la fuente del
objeto en una pestaña) y dos entradas en el menú "Más acciones": **Guardar snapshot de fuentes…** y
**Comparar con snapshot…** (el resultado va a Salida y el script se abre en una pestaña para revisarlo).

**Verificación contra Oracle (conexión del keyring), sonda temporal:**

```
snapshot objects: 4710
identical after a round trip: 4710 of 4710
changed: AJUSTAR_A_MONTO (FUNCTION), first line 34
missing detected: yes
todos in the schema: 16
  TODO PACKAGE BODY PKG_APP:5955 -- TODO: Guardar BLOB en storage (DOCUMENTO_CLIENTE o filesystem)
P4 PROBE OK
```

**Evidencia en la app instalada** (22:33, firmada autofirmada):

| Comando | Resultado |
|---|---|
| `node e2e/phase-p4.mjs` | `P4 CHECKS OK (5)`: quinta vista To-Do con 16 notas (`TODO|FIXME … :línea`), clic abre la fuente; el menú ofrece las dos entradas; `save_sources_snapshot` guarda 4710 objetos y `compare_with_snapshot` los da idénticos con script vacío |
| `phase-p1` / `phase-p2` / `phase-p3` | 15 / 11 / 4 |
| `axe-cdp.mjs` | 0 violaciones, **9 vistas** × 2 temas (incluye To-Do) |
| `phase-f1` / `phase-f2` / `phase-f5` / `tree` / `env` / `keyboard-walk` | 19 / 3 / 39 / 11 / 12 / 28 paradas |
| `commands-check` / `flow` | mismo digest / IDENTICAL |
| capturas 1366×768, 1024×700 y 1024×640, claro y oscuro | `docs/screenshots/parity-p4-todos-*.png`, 16 filas, sin desbordes |
| `cargo fmt` / `clippy -D warnings` / `check -p sqlt-desktop` / `test` | 0 / 0 / 0 / 0 (19 ok) |
| `npm run build` / `npm test` | 0 / **98 passed** |

**Límites declarados:** el comparador cubre unidades de programa (paquetes, procedimientos, funciones,
triggers, tipos y cuerpos), no tablas ni vistas (su DDL exige MetaDictionary y sería lento por objeto); el
diálogo nativo de archivos no lo puede manejar CDP, así que la prueba invoca los comandos con una ruta fija y
solo verifica la presencia de las entradas del menú.

### P5 — cerrada (2026-11-26)

**Infraestructura nueva en `sqlt-oracle`:** `Session::execute_block(block, binds, out_positions)` corre un
bloque anónimo y **lee sus binds de salida** con `Statement::bind_value` (lo que `execute_with` descartaba).
Es la base que P6 necesita para `DBMS_DEBUG`. Respeta el modo solo lectura (`guard`).

**Profiler (`crates/sqlt-app/src/profiler.rs`):** arranca `DBMS_PROFILER`, ejecuta la sentencia, **siempre** lo
detiene (aunque la sentencia falle, para no perfilar lo que venga después) y lee las líneas más costosas de esa
corrida desde `PLSQL_PROFILER_DATA`/`_UNITS` con el texto de `USER_SOURCE`. Devuelve unidad, línea, nanosegundos,
ocurrencias y texto, ordenado por costo. Formato de duración con test propio.

**Hallazgo de entorno (real y bloqueante):** `DBMS_PROFILER` está instalado y las tres tablas del profiler
existen en el esquema, pero **faltaba la secuencia `PLSQL_PROFILER_RUNNUMBER`** que crea `profload.sql`; sin
ella `start_profiler` falla con `ORA-06528` (comprobado con tres variantes de bind, incluida sin binds). La app
la crea si falta, de forma idempotente, y lo informa en el estado (`ProfileResult.prepared`).

**UI:** entrada **Perfilar sentencia** en "Más acciones": corre la sentencia del editor bajo el profiler y deja
el resultado por línea en la pestaña Salida, con el resumen `N linea(s) con tiempo · X ms`.

**Verificación contra Oracle (sonda temporal):**

```
run 2: 7 line(s), 5.516 ms total
       1911987 ns x20000 SQLT_P5_PROBE:8 l := l + j * 2;
       1584397 ns x20000 SQLT_P5_PROBE:5 l := l + i;
       1039900 ns x20001 SQLT_P5_PROBE:4 for i in 1 .. p_rows loop
        847094 ns x20001 SQLT_P5_PROBE:7 for j in 1 .. p_rows loop
P5 PROBE OK
```

**Evidencia en la app instalada** (22:50, firmada autofirmada):

| Comando | Resultado |
|---|---|
| `node e2e/phase-p5.mjs` | `P5 CHECKS OK (3)`: el menú ofrece **Perfilar sentencia**; perfilar `begin sqlt_p5_e2e(20000); end;` devuelve `4 linea(s) con tiempo · 5.325 ms` y la salida muestra `SQLT_P5_E2E:1 5.162 ms x80004 …`; la prueba borra su procedimiento y las filas del profiler |
| `phase-p1` / `phase-p2` / `phase-p3` / `phase-p4` | 15 / 11 / 4 / 5 |
| `axe-cdp.mjs` | 0 violaciones, 9 vistas × 2 temas |
| `phase-f1` / `phase-f2` / `phase-f5` / `tree` / `env` / `keyboard-walk` | 19 / 3 / 39 / 11 / 12 / 28 paradas |
| `commands-check` / `flow` | mismo digest / IDENTICAL |
| capturas 1366×768, 1024×700 y 1024×640, claro y oscuro | `docs/screenshots/parity-p5-profiler-*.png`, la salida del profiler visible en las seis |
| `cargo fmt` / `clippy -D warnings` / `check -p sqlt-desktop` / `test` | 0 / 0 / 0 / 0 (19 ok) |
| `npm run build` / `npm test` | 0 / **98 passed** |

**Límites declarados:** el profiler mide **una** sentencia por corrida y no agrega varias corridas; si la
sentencia no es PL/SQL, `DBMS_PROFILER` no registra líneas y la app lo dice. Las filas del profiler quedan en
las tablas `PLSQL_PROFILER_*` del usuario (la app no las borra en uso normal).

### P6 — pre-vuelo del Debugger (2026-11-26): bloqueada por alcance

Antes de escribir el debugger se probó qué permite `DBMS_DEBUG` en `anamnesis@desa02`. Todo lo que sigue es
evidencia ejecutada, no suposición:

| Prueba | Resultado |
|---|---|
| `all_tab_privs` para `DBMS_DEBUG` | 1 concesión, **a PUBLIC**: se puede llamar |
| `alter session set plsql_debug = true` + compilar | funciona; la unidad queda con `plsql_debug = TRUE` (las breakpoints serían posibles) |
| `dbms_debug.initialize` | **funciona** con bind de salida `VARCHAR2` (devuelve `0170E3870001`); con `NUMBER` daba `ORA-06502` |
| `dbms_debug.set_breakpoint(owner, program, null, 4)` | `PLS-00221`: la firma real pide más parámetros (incluye un `OUT`), así que cada llamada necesita descubrir su firma exacta y bindear las salidas |
| `dbms_debug.debug_on` | devuelve ok, pero deja la sesión **dentro del debug loop** |
| `dbms_debug.debug_off` | **se cuelga** (900 s, proceso terminado a mano) porque no hay ningún debugger adjunto que la libere |

**Consecuencia de diseño:** un debugger real necesita **dos sesiones OCI** —una de ellas en un hilo, bloqueada en
el debug loop como objetivo— y un bucle de comandos en la otra (`continue`, `step`, `get_value`), cada uno con
sus propios `OUT` y con timeout. Además hay que garantizar `debug_off` o matar la sesión objetivo al salir: la
prueba demostró que sin debugger adjunto la sesión queda colgada.

Eso es un sub-proyecto de varios turnos (protocolo de 8+ llamadas con firmas por descubrir, hilo objetivo,
UI de paso/variables y recuperación ante cuelgues), no una fase más de las que veníamos cerrando en un turno.
Se deja **P6 sin implementar** y se pide la decisión del dueño: invertir los turnos que haga falta en el
debugger, o sacarlo del alcance y seguir con P7–P10. La sesión de prueba quedó terminada y no hay objetos
creados (el procedimiento de prueba se borró).

### P6 — en progreso: el protocolo de dos sesiones funciona (2026-11-26)

Prototipo ejecutado contra `anamnesis@desa02` (dos sesiones OCI, `abort_on_timeout` en el objetivo):

```
debug id: 007CC8190001
target debug_on: Ok("ok")
synchronize: [Some("0"), None, Some("0"), Some("2"), None]     # ret=0, line=NULL, terminated=0, reason=2
set_breakpoint: ret=Some("16") number=None                     # 16 = error_bad_handle
target run: Ok("ok") after 1.2128463s
step 0: [Some("0"), None, Some("1"), Some("25"), None]         # ret=0, terminated=1, reason=25
target finished cleanly
P6 PROTO OK
```

Lo que quedó probado:
- `DBMS_DEBUG.INITIALIZE` (VARCHAR2) y `DEBUG_ON` funcionan; **no hace falta matar la sesión**: con
  `set_timeout_behaviour(abort_on_timeout)` el objetivo termina limpio.
- `ATTACH_SESSION` + `SYNCHRONIZE` dejan el objetivo suspendido en el evento de arranque (ret=0, terminated=0).
- `CONTINUE` desde la sesión debugger corre el programa hasta el final y devuelve `terminated=1`: **el ida y
  vuelta entre las dos sesiones no se cuelga**.
- La API real se leyó del diccionario (`all_arguments`) y del texto del paquete (`all_source`):
  `set_breakpoint(program, line#, breakpoint# OUT)` — mi primer intento omitía `line#`.

Lo que falta para cerrar P6: los **valores numéricos de `breakflags`** (para paso a paso) y por qué
`set_breakpoint` devuelve `error_bad_handle` (16) sobre un procedimiento propio; después, el módulo
`debugger.rs` con `start/step/stop/watch`, la UI de paso y variables, y las verificaciones de la compuerta.
El prototipo se borró; la evidencia queda acá.

### P6 — cerrada (2026-11-26)

**Qué cambió:** `crates/sqlt-app/src/debugger.rs` implementa el debugger sobre `DBMS_DEBUG` con las **dos
sesiones** que exige el paquete: `Debugger::start` abre una conexión OCI aparte, la arma con
`set_timeout_behaviour(abort_on_timeout)` y `debug_on`, corre la sentencia ahí en un hilo, y desde la sesión de
la app hace `attach_session` + `synchronize`. `step(Over|Into|Out)` usa `continue` con `break_next_line` (32),
`break_any_call` (12) y `break_any_return` (512); `watch(name)` lee variables del frame con `get_value`; y
`stop` hace `detach_session` + cierra la conexión objetivo, que además libera el hilo si quedó suspendido.
Los RECORD del paquete se bindean desde **bloques PL/SQL envoltorio** (OCI no puede bindear records) y
`Session::execute_block` devuelve `Option<String>` porque `DBMS_DEBUG` deja campos en NULL.

**Lo que costó y quedó documentado:** el namespace de un PROCEDURE es **1** (usé 2, de ahí `error_bad_handle`
16); `set_breakpoint` devuelve **1 = error_deferred** cuando se pide antes de que el programa corra — se acepta
y se instala al sincronizar, así que no es un error; `set_timeout` es función, `set_timeout_behaviour` es
procedimiento; y el primer `continue` desde el bloque anónimo no entra en la unidad: el camino confiable es
breakpoint + paso.

**UI:** entrada **Depurar sentencia** en "Más acciones" y, mientras hay sesión, una tira de depuración con la
posición (`UNIDAD:línea · motivo`), **Paso a paso / Entrar / Salir / Detener** y un campo de **Variable** con
los valores observados. Cada paso se registra en Salida.

**Verificación contra Oracle (sonda temporal, ya borrada):**

```
start: interpreter starting :-1 terminated=false
set_breakpoint: [Some("0"), Some("1")]
step 0: breakpoint SQLT_P6_DBG:4 terminated=false status=0   L = Some("0")
step 1: new line SQLT_P6_DBG:5   ...                          L = Some("1")
step 2: new line SQLT_P6_DBG:6   ...                          L = Some("1")
step 3: new line SQLT_P6_DBG:5   ...                          L = Some("2")
step 4: new line SQLT_P6_DBG:6   ...                          L = Some("2")
step 5: new line SQLT_P6_DBG:5   ...                          L = Some("4")
```

**Evidencia en la app instalada** (23:37, firmada autofirmada):

| Comando | Resultado |
|---|---|
| `node e2e/phase-p6.mjs` | `P6 CHECKS OK (6)`: el menú ofrece Depurar; la sesión arranca (`interpreter starting`); el breakpoint se acepta; el paso frena en `SQLT_P6_E2E:4 · breakpoint`; `L=0` en la tira; Detener quita la tira |
| `phase-p1` … `phase-p5` | 15 / 11 / 4 / 5 / 3 |
| `axe-cdp.mjs` | 0 violaciones, 9 vistas × 2 temas (la tira de depuración no está en el barrido: requiere una sesión activa; declarado) |
| `phase-f1` / `phase-f2` / `phase-f5` / `tree` / `env` / `keyboard-walk` | 19 / 3 / 39 / 11 / 12 / 28 paradas |
| `commands-check` / `flow` | mismo digest / IDENTICAL |
| capturas 1366×768, 1024×700 y 1024×640, claro y oscuro | `docs/screenshots/parity-p6-debugger-*.png`, con `SQLT_P6_SHOT:4 · breakpoint` visible y sin desbordes |
| `cargo fmt` / `clippy -D warnings` / `check -p sqlt-desktop` / `test` | 0 / 0 / 0 / 0 (19 ok; +3 tests del debugger) |
| `npm run build` / `npm test` | 0 / **98 passed** |

**Límites declarados:** la unidad tiene que estar compilada **con información de debug** (`PLSQL_DEBUG=TRUE` en
la sesión que la compila; la app lo activa al iniciar la depuración, no recompila). No hay UI para poner
breakpoints (el comando existe y lo usa la prueba); una sesión de depuración por vez; el timeout es de 15 s y
al detener se cierra la sesión objetivo.

### P7 — cerrada (2026-11-27)

**Qué cambió:** `crates/sqlt-app/src/reports.rs` (nuevo) con dos cosas:

- **Informes**: archivos `.sql` en `<config>/reports`, con los parámetros `&nombre`/`&&nombre` que el
  ejecutor ya sustituye (`parameters` los extrae en orden, sin repetir e ignorando literales). Guardar, listar
  (ordenado) y borrar, con nombres saneados (sin separadores ni `..`). **4 tests.**
- **Explorador de archivos**: `list_dir` (carpetas primero, ocultos fuera, tamaño incluido) y las ubicaciones
  del usuario en `locations.json`, con `Documentos` y el perfil como valores por defecto.

Comandos: `list_reports`, `save_report`, `delete_report`, `list_file_locations`, `add_file_location`,
`list_directory`, `pick_folder`. UI: sexta vista lateral **Archivos** (ubicaciones + navegación + abrir `.sql`
en el editor, botón agregar carpeta) y diálogo **Informes…** en el menú (ejecutar, eliminar, guardar el editor
con un nombre y el conteo de parámetros). El diálogo se marcó `role="dialog"` con etiqueta porque `axe` lo
reportaba fuera de todo landmark.

**Evidencia en la app instalada** (27/11, firmada autofirmada):

| Comando | Resultado |
|---|---|
| `node e2e/phase-p7.mjs` | `P7 CHECKS OK (9)`: la barra tiene Archivos; lista las ubicaciones y el contenido de una carpeta (carpetas primero); el menú abre Informes; guardar el editor crea `sqlt_p7_e2e` y aparece en la lista; **Ejecutar** corre su SQL (`1 fila · 4 ms`); un informe con `&n` muestra `· 1 parámetro(s)`; borrar deja la lista vacía |
| `axe-cdp.mjs` | **0 violaciones, 11 vistas × 2 temas** (incluye Archivos e Informes) |
| `phase-p1` … `phase-p6` | 15 / 11 / 4 / 5 / 3 / 6 |
| `phase-f1` / `phase-f2` / `phase-f5` / `tree` / `env` / `keyboard-walk` | 19 / 3 / 39 / 11 / 12 / 29 paradas |
| `commands-check` / `flow` | mismo digest / IDENTICAL |
| capturas 1366×768, 1024×700 y 1024×640, claro y oscuro | `docs/screenshots/parity-p7-files-*.png` (66 entradas, sin desbordes) y `parity-p7-reports-*.png` |
| `cargo fmt` / `clippy -D warnings` / `check -p sqlt-desktop` / `test` | 0 / 0 / 0 / 0 (19 ok; +4 tests de informes) |
| `npm run build` / `npm test` | 0 / **98 passed** |

**Límites declarados:** un informe con parámetros abre el diálogo de sustitución por `window.prompt`, que CDP
no puede manejar, así que la prueba verifica el conteo de parámetros y ejecuta uno sin parámetros; no hay
exportación de informes a PDF/HTML ni vista de informe paginada.

### P8 — cerrada (2026-11-27)

**Proyectos (`crates/sqlt-app/src/projects.rs`):** un JSON por proyecto en `<config>/projects` con
`{ name, schema, notes, files: [{title, sql}], objects }`; `files` son los buffers abiertos y `objects` sale de
las pestañas de DDL abiertas (`TIPO nombre`). Guardar, listar ordenado (un archivo roto se salta en vez de
romper la lista), cargar y borrar, con nombres saneados. **4 tests** (ida y vuelta, orden y archivo roto,
nombres inseguros, proyecto mínimo sin campos opcionales).

**Ventana de comandos:** diálogo con un log tipo SQL*Plus y una línea de entrada; cada Enter corre la sentencia
con el **mismo** `api.execute` del editor (columnas, filas, tiempo y errores al log). No se agregó una pestaña
inferior nueva: la decisión de F5 de dejar cuatro pestañas sigue en pie.

**UI:** dos entradas nuevas en "Más acciones": **Proyectos…** (abrir / eliminar / guardar el proyecto actual con
nombre) y **Ventana de comandos**. Abrir un proyecto reemplaza los buffers y el esquema.

**Hallazgo propio corregido en la fase:** `phase-p1` era intermitente (2 de 3 fallos): el primer `F5` podía
caer antes de que `isConnected` se propagara y la ejecución devolvía "sin conexión". La prueba ahora espera a
que el botón **Ejecutar** quede habilitado; 3 corridas seguidas en verde.

**Evidencia en la app instalada** (27/11, firmada autofirmada):

| Comando | Resultado |
|---|---|
| `node e2e/phase-p8.mjs` | `P8 CHECKS OK (5)`: guardar el buffer actual como `sqlt_p8_e2e` → aparece con `1 archivo(s)`; cambiar el editor y abrir el proyecto **restaura** `select 1 as proyecto from dual`; la ventana de comandos corre `select 7 as siete from dual` y muestra `SIETE / 7 / 1 fila(s) · 1 ms`; borrar lo elimina |
| `axe-cdp.mjs` | **0 violaciones, 13 vistas × 2 temas** (incluye Proyectos y Ventana de comandos) |
| `phase-p1` … `phase-p7` | 15 / 11 / 4 / 5 / 3 / 6 / 9 |
| `phase-f1` / `phase-f5` / `keyboard-walk` | 19 / 39 / 29 paradas |
| `commands-check` / `flow` | mismo digest / IDENTICAL |
| capturas 1366×768, 1024×700 y 1024×640, claro y oscuro | `docs/screenshots/parity-p8-command-window-*.png` (log con 28–29 líneas) y `parity-p8-projects-*.png` |
| `cargo fmt` / `clippy -D warnings` / `check -p sqlt-desktop` / `test` | 0 / 0 / 0 / 0 (19 ok; +4 tests de proyectos) |
| `npm run build` / `npm test` | 0 / **98 passed** |

**Límites declarados:** el proyecto guarda los buffers y el esquema, no ejecuta "compilar todos los objetos"
ni compara versiones; la ventana de comandos ejecuta una sentencia por Enter (no interpreta `SQL*Plus`
como `SET SERVEROUTPUT`), y los `objects` del proyecto son los que estaban abiertos en pestañas de DDL.

### P9 — cerrada (2026-11-27)

**Formularios DDL (`web/src/ddl/forms.ts`):** generan el texto y **nunca ejecutan**: la sentencia va al
editor. Tabla con columnas (nombre, tipo, nula, PK) y `CONSTRAINT <tabla>_PK PRIMARY KEY`; secuencia con
inicio, incremento, `maxvalue`/`nomaxvalue`, `cycle` y caché. Identificadores citados solo cuando hace falta y
rechazo de comillas, tipos inválidos, columnas duplicadas, tabla sin columnas y `maxvalue` no numérico.
**5 tests de vitest.**

**Scheduler (`crates/sqlt-app/src/scheduler.rs`):** vista de solo lectura sobre `DBMS_SCHEDULER`
(`Tareas programadas`, séptima vista lateral) con nombre, habilitada/deshabilitada, estado, intervalo y próxima
ejecución. `user_scheduler_jobs` no tiene columna `OWNER` (se usa el pseudo-campo `user`), corregido al probar.

**UI:** **Nuevo objeto…** en "Más acciones" (selector Tabla/Secuencia, nombre, editor de columnas, preview y
"Insertar en el editor") y la vista **Tareas programadas**.

**Evidencia en la app instalada** (27/11, firmada autofirmada):

| Comando | Resultado |
|---|---|
| `node e2e/phase-p9.mjs` | `P9 CHECKS OK (6)`: la vista lista un job real creado por la prueba (`SQLT_P9_E2E … deshabilitada`); el formulario de tabla previsualiza `create table SQLT_P9_E2E ( ID NUMBER NOT NULL, CONSTRAINT SQLT_P9_E2E_PK PRIMARY KEY (ID) );`; **insertar** deja el DDL en el editor y el estado pasa a "Insertar en el editor" (no se ejecutó); el de secuencia produce `create sequence SQLT_P9_SEQ start with 1 increment by 1 nomaxvalue nocycle cache 20;`. La prueba borra su job |
| `axe-cdp.mjs` | **0 violaciones, 15 vistas × 2 temas** (incluye Tareas programadas y Nuevo objeto) |
| `phase-p1` … `phase-p8` | 15 / 11 / 4 / 5 / 3 / 6 / 9 / 5 |
| `phase-f1` / `phase-f5` / `keyboard-walk` | 19 / 39 / 30 paradas |
| `commands-check` / `flow` | mismo digest / IDENTICAL |
| capturas 1366×768, 1024×700 y 1024×640, claro y oscuro | `docs/screenshots/parity-p9-scheduler-*.png` (108 tareas) y `parity-p9-new-object-*.png` |
| `cargo fmt` / `clippy -D warnings` / `check -p sqlt-desktop` / `test` | 0 / 0 / 0 / 0 (19 ok) |
| `npm run build` / `npm test` | 0 / **103 passed** (5 nuevos de DDL) |

**Dos defectos propios corregidos en la fase:** la consulta del scheduler usaba `owner` sobre
`user_scheduler_jobs` (no existe); y la lista de 108 tareas es un área desplazable sin contenido enfocable
(`scrollable-region-focusable`), ahora `tabIndex={0}` con `role="group"`.

**Límites declarados:** el formulario cubre tablas y secuencias (no índices, constraints ni vistas); no crea un
objeto directamente, por diseño. El scheduler es de solo lectura: no crea, habilita ni ejecuta jobs.

### P10 — cerrada (2026-11-27). Goal completo: P1–P10.

**Cliente (`crates/sqlt-app/src/ai.rs`):** compatible con OpenAI (`POST {base}/chat/completions`), sin
dependencia de un proveedor concreto (sirve OpenAI, un proxy, Ollama o vLLM). `AiConfig { baseUrl, model,
timeoutSecs }` en `<config>/ai.json`; **la clave vive en el keyring del sistema** (`com.sqltools.rust` /
`ai-key`) y nunca se escribe en un archivo ni viaja a la UI. Siete funciones predefinidas (crear, completar,
corregir, explicar, comentar, revisar, optimizar) con su instrucción; timeout configurable (60 s por defecto);
sin telemetría y **nada sale de la máquina hasta que el usuario ejecuta**. Dependencia nueva: `ureq` (TLS +
JSON). **7 tests**, incluido uno contra un **proveedor simulado local** que verifica la URL, el `Bearer`, el
modelo y el cuerpo enviado, sin gastar cuota ni usar credenciales.

**UI:** diálogo **Asistente AI…** con selector de función, el código o pedido (prefijado con el documento),
respuesta, **Insertar en el editor**, y ajustes del proveedor (URL base, modelo y clave que se guarda en el
keyring; el campo solo dice si ya hay una).

**Evidencia en la app instalada** (27/11 01:33, firmada autofirmada), contra el mock local:

| Comando | Resultado |
|---|---|
| `node e2e/phase-p10.mjs` | `P10 CHECKS OK (9)`: el menú ofrece el asistente; el diálogo abre con su área de respuesta; los ajustes guardan URL base, modelo y clave; **la respuesta del proveedor aparece** (`select 42 as respuesta from dual`); la petición fue a `/v1/chat/completions` con `Authorization: Bearer test-key`; el cuerpo lleva el código y la instrucción de "explain"; la respuesta se inserta en el editor; **la configuración no contiene la clave**. La prueba borra la clave y repone el proveedor por defecto |
| `axe-cdp.mjs` | **0 violaciones, 16 vistas × 2 temas** (incluye el asistente) |
| `phase-p1` … `phase-p9` | 15 / 11 / 4 / 5 / 3 / 6 / 9 / 5 / 6 |
| `phase-f1` / `phase-f5` / `keyboard-walk` | 19 / 39 / 30 paradas |
| `commands-check` / `flow` | mismo digest / IDENTICAL |
| capturas 1366×768, 1024×700 y 1024×640, claro y oscuro | `docs/screenshots/parity-p10-assistant-*.png` |
| `cargo fmt` / `clippy -D warnings` / `check -p sqlt-desktop` / `test` | 0 / 0 / 0 / 0 (19 ok; 7 tests nuevos) |
| `npm run build` / `npm test` | 0 / **103 passed** |

**NO VERIFICADO:** ninguna llamada real a un proveedor (no hay clave del dueño); todo lo anterior es contra el
mock local. Cancelar una consulta en vuelo no aborta el socket: el resultado se descarta y el timeout lo corta
(declarado).

## Cierre del goal P1–P10

| Fase | Commit | Checks propios |
|---|---|---|
| P1 | `19b1e8f` | 15 |
| P2 | `d6b44c4` | 11 |
| P3 | `dc135ee` | 4 |
| P4 | `96956f4` | 5 |
| P5 | `6873a66` | 3 |
| P6 | `e0bd78e` | 6 |
| P7 | `c3683cd` | 9 |
| P8 | `ef12433` | 5 |
| P9 | `2ff99c0` (+`48e740d`) | 6 |
| P10 | este commit | 9 |

Compuerta del último cierre: axe **0 en 16 vistas × 2 temas**, `cargo fmt/clippy -D warnings/check/test` en 0,
vitest **103**, `flow` **IDENTICAL**, capturas en los tres tamaños, inventario de paridad actualizado.

