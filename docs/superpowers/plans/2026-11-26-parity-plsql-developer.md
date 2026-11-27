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

