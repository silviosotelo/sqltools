# UI workbench (propuesta B) — plan de implementación

> **Para quien ejecute:** usar `superpowers:executing-plans` (o `subagent-driven-development`). Los pasos usan casillas `- [ ]`. Se ejecuta **en orden** y una fase no se cierra sin su verificación con salida real.

**Objetivo:** reemplazar la interfaz de `rust/web` por la propuesta B en cinco fases, sin romper lo que hoy funciona.

**Arquitectura:** marco nuevo (`TitleBar`, `ActivityBar`, `SidePanel`, `StatusBar`) alrededor del contenido actual (F1); extracción de componentes visuales con el estado en `App.tsx` (F2); lo nuevo (árbol, conexiones) nace como componente + hook propio (F3, F4); vistas laterales y paleta anclada (F5). Backend: un campo en `ConnectionConfig`, dos comandos nuevos (`count_objects`, `transaction_pending`).

**Stack:** React 19 + TypeScript + Vite 6, CodeMirror 6, Tauri v2, Rust (`oracle` 0.6.3). Nuevas dependencias: `@fontsource/ibm-plex-sans`, `@fontsource/jetbrains-mono`, `@phosphor-icons/react`; dev: `vitest` y `axe-core` (ambas desde la Tarea 0).

**Spec:** `docs/superpowers/specs/2026-11-26-ui-workbench-design.md`. **Maqueta de referencia:** `docs/ui-proposals/b-workbench/index.html` (+ `shots/`).

## Restricciones globales

- Código, identificadores, errores y tests en inglés; textos de UI solo por `rust/web/src/i18n.tsx` (es/en).
- Colores solo por tokens CSS; ningún hex literal fuera de `:root` / `[data-theme="dark"]`.
- **Se omite** (no existe en la app): Autocommit, filtro de filas, botón Copiar, pista `@` de la paleta. Entorno solo visual.
- No se toca: ejecución, streaming de resultados, pestañas de documentos, los 226 comandos y atajos.
- **PowerShell (gotchas ya vividos):** no usar `Set-Content -Encoding utf8` ni `Get-Content -Raw` para editar archivos (agrega BOM / rompe acentos); editar con las herramientas de edición o con Python. Los scripts largos *inline* los bloquea el antivirus: guardarlos en `%TEMP%\opencode\` y ejecutarlos. Para mirar una captura, copiarla con nombre nuevo (el visor cachea por ruta). Los botones solo-icono se buscan por `title`, no por texto.
- Verificación en la app: lanzar con `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`; build de desarrollo `rust/target/debug/sqlt-desktop.exe`, instalada `%LOCALAPPDATA%\SQLTools\sqlt-desktop.exe`. Entorno Oracle de pruebas: `SQLT_USER=anamnesis`, `SQLT_ALIAS=desa02`, `SQLT_OCI_DIR=C:\instantclient_23_5`, `SQLT_TNS_ADMIN=C:\instantclient_23_5\network\admin` (la contraseña va por variable de entorno, nunca se escribe en un archivo).

## Compuerta de cierre de CADA fase

Desde `rust/` (y `rust/web` donde corresponda), todo con salida real:

```
cargo fmt --all -- --check
cargo clippy --workspace --exclude sqlt-desktop --all-targets -- -D warnings
cargo check -p sqlt-desktop            # el crate de escritorio no entra en clippy/test
cargo test --workspace --exclude sqlt-desktop
npm --prefix rust/web run build
npm --prefix rust/web test            # vitest, desde la Tarea 0
node rust/web/e2e/flow.mjs            # CDP en la app: conectar, explorar, ejecutar, Ctrl+F12
node rust/web/e2e/axe-cdp.mjs         # axe: 0 violaciones en las pantallas de la fase, claro y oscuro
```

Más: reconstruir e instalar el NSIS (`npx @tauri-apps/cli@^2 build` en `rust/desktop` con `TAURI_SIGNING_PRIVATE_KEY`, firmado como en la política de firma) y repetir `flow.mjs` contra la instalada; verificar 1366×768 y 1024×700 sin desbordes; anotar la evidencia al final de la fase en este archivo; commit + push a `rust`; `MEM-SAVE`.

## Enmiendas de la revisión externa (obligatorias)

El `advisor` interno no tuvo cupo (límite de OpenCode Go); la revisión se hizo con `codex exec` (gpt-6.2-sol, solo lectura, el 2026-11-26) y se verificó contra las pruebas del plan. Lo que **bloqueaba** y cómo queda resuelto:

1. **Contraste efectivo (F1).** Conservar nombres de tokens mantiene las referencias pero cambia el contraste de todos sus usos, y `--line-strong` no llega a 3:1 como borde de control en ningún tema. → Se agrega el token **`--control-border`** (≥ 3:1 contra `--surface`, claro `#8a93a2` / oscuro `#6b7482`) para bordes de inputs, selects y botones; `--line-strong` queda para separadores. Se agrega `web/src/tokens.test.ts` (vitest, Tarea 0): lee los tokens de `styles.css` y exige contraste ≥ 4,5:1 en `text/muted/faint/accent-text` contra sus superficies y ≥ 3:1 en `control-border`, en ambos temas, **antes** de dar F1 por cerrada. También se prueba el panel abierto con los 21 botones a **1024×640** (el mínimo declarado).
2. **F2 preserva estado.** Cambiar de vista lateral no debe perder el documento activo, el historial de deshacer ni resultados en curso: se agrega al `flow.mjs` un paso que alterna Objetos/Conexiones y exige texto del editor, pestañas y filas de la grilla iguales.
3. **Árbol virtualizado (F3).** Se agregan `aria-level`, `aria-setsize` y `aria-posinset` calculados sobre los hermanos del árbol **filtrado completo**, no sobre la ventana visible. La fila enfocada se mantiene montada aunque salga de la ventana (o el foco se traslada antes de desmontarla). Los conteos, los errores y `loadAll` quedan protegidos por una **generación de conexión/esquema** además del `requestId` por carga. `loadAll` corre **de a un pedido** (la sesión es compartida) y cancelar detiene los pedidos nuevos sin bloquear la ejecución SQL. Pruebas: teclado con scroll, filtro y colapso (vitest sobre `nextFocus` + CDP).
4. **DTO de conteos (F3).** Ya reflejado en T3.1: struct `ObjectCount` (camelCase) y paridad con `list_objects` en los 9 tipos.
5. **Persistencia del entorno (F4).** La deserialización de `ConnectionConfig` **no** llama a `Environment::validated`: se aplica en la **ruta de guardado** (`save_connection`). Se prueba contra el almacén real: actualizar desde un archivo anterior sin el campo, editar la etiqueta y **quitarla** (vuelve a `None`), y eliminar la conexión.
6. **Transacción (F5, T5.4).** `LOCAL_TRANSACTION_ID` indica un XID local **asignado**, no "hay cambios": un `SELECT … FOR UPDATE` con locks da positivo, una transacción `READ ONLY` no, y un DDL provoca commit implícito (antes y después, aunque falle). Por eso: el texto de la UI es **"Transacción abierta"** (no "cambios sin confirmar"), la consulta corre en la misma sesión física bajo el mismo `execution lock`, se descartan respuestas de sesiones reemplazadas, y un timeout o error se muestra como **desconocido** (sin indicador). Pruebas en 12.1 con el usuario real: DML, DDL exitoso y fallido, `FOR UPDATE`, `READ ONLY`, cancelación, carrera con otra ejecución, y rollback seguido de un **segundo** DML que reactiva el indicador.
7. **Compuerta y evidencia.** Cada cierre registra **qué binario y qué commit** recibieron la prueba CDP; el golden normaliza campos volátiles y la fase lista sus **cambios esperados**; en F5 se compara contra el golden **IDs, atajos y disponibilidad de los 226 comandos** (`list_commands`) y se comprueba el efecto de cada una de las 21 acciones retiradas, además de streaming, cancelación y retorno del foco de la paleta; `axe-cdp.mjs` **falla si una vista esperada no llegó a abrirse** y se revisan los resultados `incomplete`.

Menores incorporados: `Ln/Col` usa **conteo de caracteres desde el inicio de la línea (base 1, un tab cuenta 1)** y se verifica al abrir o cambiar de documento además de al mover el cursor; el límite de 12 caracteres de la etiqueta se mide en **valores escalares Unicode** (`chars().count()` en Rust, `[...s].length` en TS) con los mismos casos de prueba en ambos lados (`ÁÉÍÓÚ`, un emoji, 12 vs 13 caracteres).

## Review Focus (modos de falla que las pruebas de cada tarea deben cubrir)

1. **Carga tardía del árbol** tras cambiar de conexión o esquema → el resultado viejo no pisa el nuevo (test del reductor, F3).
2. **Conexión guardada sin `environment`** sigue abriendo (test Rust, F4).
3. **Contraste de cada color de entorno** en ambos temas (test que lee los tokens, F4).
4. **Teclado en un árbol virtualizado**: la fila enfocada puede desmontarse al scrollear; el foco no se pierde (test de `nextFocus` + CDP, F3).
5. **Ventana de 1024×700**: la barra de título y la barra de actividad no se cortan (CDP, F1).
6. **Tema claro y oscuro del editor** leen los mismos tokens que el resto (CDP con captura, F1).
7. **Transacción**: el indicador refleja la verdad de Oracle y no una heurística (test de integración, F5).

---

## Tarea 0 — Herramientas de verificación reproducibles

**Archivos:** crear `rust/web/e2e/cdp.mjs`, `rust/web/e2e/flow.mjs`, `rust/web/e2e/axe-cdp.mjs`, `rust/web/vitest.config.ts`; modificar `rust/web/package.json` (dev: `axe-core`, `vitest`; script `"test": "vitest run"`).

**Interfaces:**
- `cdp.mjs` exporta `connect(port=9222)` → `{ evaluate(expr), key(k, code, vk, mods), click(selector), clickTitle(title), clickText(text), waitFor(fn, ms), screenshot(path), close() }`, usando solo `fetch`/`WebSocket` de Node 22 (sin dependencias).
- `flow.mjs`: recorre conectar → cargar objetos → abrir una tabla → `Ctrl+F12` → ejecutar `select * from dual`, e imprime **un JSON único** (`snapshot`) con hechos del DOM: títulos de pestañas, filas de la grilla, texto de estado, conteo de objetos, texto del chip de conexión. Opción `--save <archivo>` guarda el JSON; `--compare <archivo>` falla (exit 1) si difiere.
- `axe-cdp.mjs`: inyecta `axe-core/axe.min.js` con `Runtime.evaluate`, corre `axe.run` en cada **vista** que se le pase (`--views id1,id2`, navegando por clics definidos en un mapa `viewSteps`) y en los dos temas; imprime `critical/serious/moderate/minor` por vista y sale con 1 si hay alguna.

- [ ] Escribir los tres scripts; `npm --prefix rust/web i -D axe-core vitest`; `vitest.config.ts` con `environment: "node"` e `include: ["src/**/*.test.ts"]`.
- [ ] Verificar contra la app actual (antes de cualquier cambio): `node rust/web/e2e/flow.mjs --save rust/web/e2e/golden-before.json` y `node rust/web/e2e/axe-cdp.mjs` sobre el estado inicial; guardar la salida real en este plan como línea base.
- [ ] Commit `test(e2e): reproducible CDP flow and axe tooling`.

---

## Fase F1 — Tokens, fuentes, iconos y marco

**Criterio:** tokens/fuentes/iconos de B, marco nuevo con las funciones actuales intactas; la barra de herramientas de 21 botones **sigue** (restilada) hasta F5; 1366×768 y 1024×700 sin desbordes.

### T1.1 Dependencias, fuentes y licencias
**Archivos:** `rust/web/package.json`, `rust/web/src/main.tsx`, crear `rust/web/public/licenses/{IBM-Plex-Sans-OFL-1.1.txt,JetBrains-Mono-OFL-1.1.txt,Phosphor-Icons-MIT.txt}` (copiar de `docs/ui-proposals/shared/licenses/`), crear `rust/THIRD_PARTY_NOTICES.md`.
- [ ] `npm --prefix rust/web i @fontsource/ibm-plex-sans @fontsource/jetbrains-mono @phosphor-icons/react`.
- [ ] En `main.tsx` importar `@fontsource/ibm-plex-sans/{400,500,600}.css` y `@fontsource/jetbrains-mono/{400,500}.css` antes de `./styles.css`.
- [ ] Verificar: `npm --prefix rust/web run build` → 0 y `rust/web/dist/assets` contiene los `.woff2`.

### T1.2 Tokens de B
**Archivos:** `rust/web/src/styles.css` (bloques `:root` y `[data-theme="dark"]`), `rust/web/DESIGN.md`.
Se **conservan los nombres actuales** y se cambian sus valores; se agregan los nuevos. Valores (claro / oscuro):

| Token | Claro | Oscuro |
|---|---|---|
| `--bg` | `#f3f4f7` | `#0e1013` |
| `--side` (nuevo) | `#eceef2` | `#12151a` |
| `--surface` | `#ffffff` | `#171a20` |
| `--surface-alt` | `#f7f8fa` | `#1c2027` |
| `--surface-sunken` | `#e3e6eb` | `#0b0d10` |
| `--line` / `--line-soft` | `#d9dde4` | `#252a32` |
| `--line-strong` | `#a9b0bc` | `#3e4551` |
| `--text` / `--text-strong` | `#1a1e26` / `#0b0e13` | `#dde1e8` / `#f5f7fa` |
| `--muted` | `#4d5666` | `#98a2b2` |
| `--faint` (nuevo) | `#586272` | `#8791a1` |
| `--accent` | `#0a7285` | `#3cc4dc` |
| `--accent-ink` (nuevo) | `#ffffff` | `#04222a` |
| `--accent-soft` / `--accent-wash` | `#d5eef3` | `#12333b` |
| `--accent-text` / `--accent-strong` (nuevo/alias) | `#075565` | `#8fe0ef` |
| `--success` | `#17794a` | `#4cc38a` |
| `--warn` / `--warn-soft` (nuevos) | `#8f5200` / `#fbeccb` | `#e3a93e` / `#35290f` |
| `--danger` / `--err-soft` (nuevo) | `#b3261e` / `#fbe4e1` | `#f28b82` / `#3a1d1b` |
| `--control-border` (nuevo, ≥ 3:1 sobre `--surface`) | `#8a93a2` | `#6b7482` |
| `--t-k` `--t-s` `--t-n` `--t-c` (sintaxis, nuevos) | `#0a6a80` `#8a4b00` `#0f7a5a` `#586272` | `#59c9de` `#e9b46a` `#7ad8b4` `#8791a1` |

Tipografía: `--font: "IBM Plex Sans", "Segoe UI", system-ui, sans-serif`, `--mono: "JetBrains Mono", Consolas, monospace`; base 13 px.
- [ ] Aplicar la tabla, `font-family`/`font-size` base y `font-variant-ligatures: none` en el editor.
- [ ] Aplicar `--control-border` a `input`, `select`, `button` (borde) y `.field`-equivalentes; escribir `web/src/tokens.test.ts` **primero** (falla con los valores viejos, pasa con los nuevos).
- [ ] Actualizar `DESIGN.md` con la tabla nueva.
- [ ] Verificar: `build` → 0; captura CDP en ambos temas sin textos ilegibles (mirar la imagen, no solo el exit code).

### T1.3 Iconos Phosphor detrás de la API actual
**Archivos:** `rust/web/src/icons.tsx`.
Interfaz: se **mantiene** `<Icon name={IconName} size?={number} />` donde `type IconName = keyof typeof ICONS` (mapa tipado: un nombre sin ícono **no compila**); internamente cada `name` mapea a un componente de `@phosphor-icons/react` (peso `regular`). Mapa mínimo: `run→Play`, `stop→Stop`, `commit→Check`, `rollback→ArrowCounterClockwise`, `format→TextAlignLeft`, `history→ClockCounterClockwise`, `plan→TreeStructure`, `stats→ChartBar`, `export→DownloadSimple`, `save→FloppyDisk`, `open→FolderOpen`, `commands→Command`, `connect→PlugsConnected`, `disconnect→Plugs`, `refresh→ArrowClockwise`, `database→Database`, `search→MagnifyingGlass`, `table→Table`, `clear→X`; y los nuevos `settings→Gear`, `keyboard→Keyboard`, `package→Package`, `view→Eye`, `function→Function`, `trigger→Lightning`, `sequence→ListNumbers`, `type→Cube`, `synonym→Link`, `procedure→TerminalWindow`, `schema→Stack`, `chevron→CaretRight`, `warning→WarningCircle`, `info→Info`, `plus→Plus`.
- [ ] Reescribir el contenido del mapa; un test de TypeScript (`tsc`) alcanza porque `name` sigue siendo `string`: agregar un log de desarrollo para nombres sin mapear.
- [ ] Verificar: `build` → 0; ningún `<Icon>` del app queda sin ícono (recorrido CDP + captura).

### T1.4 Editor: tema desde tokens + posición del cursor
**Archivos:** `rust/web/src/Editor.tsx`.
- [ ] Reemplazar los hex del tema oscuro y agregar un tema claro con `HighlightStyle` que use `var(--t-k)`, `var(--t-s)`, `var(--t-n)`, `var(--t-c)`, `var(--surface)`, `var(--text)`, `var(--surface-alt)` (una sola fuente de verdad con el resto de la app).
- [ ] Agregar prop `onCursor?: (pos: { line: number; col: number }) => void` disparada por `EditorView.updateListener` cuando cambia la selección principal (línea y columna base 1).
- [ ] Verificar: captura CDP del editor en claro y oscuro; `Ln/Col` cambian al mover el cursor (CDP con flechas).

### T1.5 Marco nuevo
**Archivos:** crear `rust/web/src/components/{TitleBar,ActivityBar,StatusBar}.tsx`; modificar `rust/web/src/App.tsx` (zona del `return`, ~2327–3226) y `styles.css` (reglas `.app`, `.topbar`, `.body`, `.status-bar`).
**Interfaces:**
```ts
type SideView = "objects" | "connections" | "history" | "stats";
TitleBar(props: { isConnected: boolean; connLabel: string; theme: ThemeMode; onTheme(): void;
  language: "es"|"en"; onLanguage(l): void; onPalette(): void; onChip(): void })
ActivityBar(props: { view: SideView; onView(v: SideView): void; onShortcuts(): void; onSettings(): void })
StatusBar(props: { dot: "ok"|"warn"|"err"|""; connLabel: string; message: string;
  eol: string; cursor: { line: number; col: number } | null; server: string })
```
- [ ] `App.tsx` guarda `sideView` (por defecto `"objects"`). En F1: `"objects"` muestra el explorador actual; `"connections"` muestra la sección actual de Conexiones; `"history"`/`"stats"` ejecutan el comportamiento actual (abren la pestaña inferior correspondiente) — F5 las reemplaza por vistas laterales.
- [ ] `.app` pasa a grilla: barra de título (40 px) / `activity (48) | side | main` / barra de estado (26 px, ancho completo). Se retiran del `.toolbar` actual marca, chip de conexión, idioma y tema (pasan a `TitleBar`); los botones de acción quedan (restilados) hasta F5. El botón de paleta de la barra de título abre la paleta modal actual (se ancla en F5).
- [ ] `StatusBar` reemplaza al `<footer className="status-bar">` interno; `showStatus` (`View.StatusBar`) sigue ocultándola. **No** muestra transacción pendiente todavía (F5).
- [ ] `rust/desktop/tauri.conf.json`: `minWidth` 1024, `minHeight` 640.
- [ ] Verificar: `flow.mjs --compare golden-before.json` (mismos hechos salvo los que cambian a propósito, anotados), `axe-cdp.mjs` en 0, capturas a 1366×768 y 1024×700 revisadas a ojo.

### T1.6 Accesibilidad heredada (la línea base de axe de la UI actual NO es 0)
La línea base (`rust/web/e2e/axe-baseline.txt`, 3 vistas × 2 temas) da **41 grupos de violaciones**, 8 reglas. Como F1 exige 0, se arreglan acá (casi todo es marcado/atributos):

| Regla (impacto) | Dónde | Arreglo |
|---|---|---|
| `aria-input-field-name` (serious) | `.cm-content` | `EditorView.contentAttributes.of({ "aria-label": t("editor"), "aria-multiline": "true" })` |
| `aria-required-attr` (critical) | `.splitter` | `aria-valuenow/min/max` sincronizados con `splitPercent` |
| `aria-required-parent` + `nested-interactive` | pestañas `.editor-tab` (roles `tab` con botón de cerrar adentro) | pestañas como `<ul>` de `<li>` con un botón de selección y otro de cerrar **hermanos** (patrón ya validado en `docs/ui-proposals`) |
| `label-title-only` | inputs Esquema y Filtro | `aria-label` con `t("schema")`/`t("filter")` |
| `color-contrast` (`.connection-chip`, `.tabbar-conn`, `.hint`) | tokens `--success`, `--muted` | los valores nuevos de T1.2 (el test de tokens lo exige) |
| `page-has-heading-one` (moderate) | raíz | `<h1 className="sr-only">` en `TitleBar` |
| `scrollable-region-focusable` | `.object-scroll` y paneles con scroll | `tabIndex={0}` + `role="region"` + `aria-label` |

- [ ] Aplicar la tabla; `node rust/web/e2e/axe-cdp.mjs` → `AXE OK` antes de cerrar F1.

**Cierre F1:** compuerta global. Evidencia anotada abajo.

---

## Fase F2 — Extraer componentes sin cambiar comportamiento

**Criterio:** `flow.mjs --compare` contra el golden tomado al cerrar F1 da **idéntico**.

### T2.0 Golden
- [ ] Con F1 cerrada: `node rust/web/e2e/flow.mjs --save rust/web/e2e/golden-f1.json`, commitear.

### T2.1 `EditorTabs`
**Archivos:** crear `components/EditorTabs.tsx`; `App.tsx` (bloque `editor-tabbar`).
`EditorTabs(props: { docs: {id,title,dirty,icon}[]; activeId: string; onSelect(id); onClose(id); onNew() })` — solo markup y eventos; el estado sigue en `App.tsx`.
- [ ] Extraer; mismos `className` y roles; verificar con `flow.mjs --compare golden-f1.json`.

### T2.2 `ResultsPanel`
**Archivos:** crear `components/ResultsPanel.tsx`; `App.tsx` (`panel-bar` + `content`).
Props: el valor de `panel`, `onPanel`, y los datos/handlers que hoy usa el bloque (columnas, filas, `scrollTop`, historial, plan, objeto, estadísticas, salida, exportar, limpiar). Se agrupan en un objeto `results` tipado en `types.ts`.
- [ ] Extraer (sin mover estado); `--compare` idéntico.

### T2.3 `SidePanel`
**Archivos:** crear `components/SidePanel.tsx`; `App.tsx`.
`SidePanel(props: { view: SideView; children: ReactNode })` — contenedor con encabezado por vista; el explorador actual entra como `children` (se reemplaza en F3).
- [ ] Extraer; `--compare` idéntico.

**Cierre F2:** compuerta global + `App.tsx` reducido (anotar líneas antes/después).

---

## Fase F3 — Árbol de objetos y conteos

**Criterio:** contadores al conectar; expandir `Paquetes` y `Sinónimos` fluido (≤ 120 nodos de fila en el DOM aun con 4.876 sinónimos); `Ctrl+F12` desde el árbol carga spec y body en pestañas; flechas funcionan.

### T3.1 Backend: `count_objects`
**Archivos:** `crates/sqlt-app/src/catalog.rs`, `desktop/src/main.rs`, `web/src/api.ts`, `web/src/types.ts`.
```rust
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct ObjectCount { pub object_type: String, pub count: u64 }
pub fn count_objects(session: &Session, schema: Option<&str>, options: QueryOptions)
    -> Result<Vec<ObjectCount>, String>
```
`select object_type, count(*) from all_objects where owner = :1 group by object_type` (owner por defecto = usuario conectado, igual que `list_objects`). Comando Tauri `count_objects(connection_id, schema)`; en TS `api.countObjects(connectionId, schema): Promise<ObjectCount[]>` con `type ObjectCount = { objectType: string; count: number }` (el DTO es un struct, **no** una tupla: una tupla llegaría como array).
- [ ] **Test primero** (`#[ignore]`, requiere Oracle): para `ANAMNESIS`, **para cada uno de los 9 tipos** del explorador (incluidos `PACKAGE` y `SYNONYM`) el conteo iguala `list_objects(.., tipo).len()`; los tipos no mostrados (`PACKAGE BODY`, `INDEX`) no aparecen. Verlo fallar sin la función, luego pasar.
- [ ] Implementar, exponer, `cargo test -p sqlt-app --test app_integration -- --ignored count` con salida real.

### T3.2 Lógica pura del árbol + vitest
**Archivos:** crear `web/src/tree/treeModel.ts`, `web/src/tree/treeModel.test.ts`, `web/vitest.config.ts`; `package.json` (`vitest`, script `"test": "vitest run"`).
```ts
type GroupStatus = "idle" | "loading" | "ready" | "error";
interface TreeRow { id: string; kind: "connection"|"schema"|"group"|"object"; depth: number;
  label: string; objectType?: string; count?: number; expanded?: boolean; status?: GroupStatus; selected?: boolean }
flattenTree(model: TreeModel): TreeRow[]
visibleWindow(total: number, scrollTop: number, viewport: number, rowHeight: number, overscan: number): { start: number; end: number }
nextFocus(rows: TreeRow[], focusedId: string, key: "ArrowUp"|"ArrowDown"|"ArrowLeft"|"ArrowRight"|"Home"|"End"): { focusId: string; toggle?: "expand"|"collapse" }
applyLoadResult(state: GroupState, requestId: number, result: ...): GroupState   // descarta requestId viejo
```
- [ ] Tests primero (nombres y aserciones): `flatten hides children of collapsed groups`; `window never exceeds viewport/rowHeight + 2*overscan rows`; `ArrowRight on a collapsed group expands, on an expanded group moves to its first child`; `ArrowLeft on a child moves to its parent`; `Home/End go to first/last visible row`; `a stale requestId result is ignored` (Review Focus 1); `focus stays valid when the focused row leaves the window` (Review Focus 4).
- [ ] Instalar `vitest`, verlos fallar, implementar, `npm --prefix rust/web test` en verde.

### T3.3 `useObjectTree`
**Archivos:** crear `web/src/tree/useObjectTree.ts`.
```ts
useObjectTree(connectionId: string | null, schema: string): {
  rows: TreeRow[]; toggle(id: string): void; retry(groupId: string): void;
  select(id: string): void; loadAll(): Promise<void>; cancelLoadAll(): void; filter: string; setFilter(f: string): void }
```
Al conectar pide `count_objects`; al expandir un grupo pide `list_objects` con un `requestId` incremental; al cambiar conexión/esquema invalida los pedidos en vuelo. Un fallo deja el grupo en `error` con `retry`.
- [ ] Implementar sobre `treeModel`; sin pruebas de React (solo `treeModel` tiene vitest); se cubre por CDP en T3.5.

### T3.4 `ObjectTree`
**Archivos:** crear `components/ObjectTree.tsx`; `App.tsx` (vista `objects` usa el árbol; se retiran la rejilla de tipos y la lista plana **después** de buscar todos los usos de `objects`, `objectType`, `chooseType`, `shownObjects`, `loadObjects`: reportar el conteo de sitios y no dejar referencias colgando).
Roles `tree`/`treeitem` con `aria-expanded`, `aria-selected`, `tabindex` rotativo; fila de 26 px; ventana virtual de `visibleWindow`; `Enter` abre (reusa `openObject`), `Ctrl+F12` carga DDL (reusa `loadDdlFor`), menú contextual actual (`menu`) se conserva. Filtro: filtra grupos ya cargados + botón "Buscar en todo el esquema" (`loadAll`, cancelable).
- [ ] Implementar; `build` → 0.

### T3.5 CI y verificación
- [ ] `.github/workflows/rust-ci.yml`: agregar a `tauri-check` (ya tiene Node) `npm --prefix rust/web test`.
- [ ] CDP (`e2e/tree.mjs`): conectar → contadores visibles; expandir `Paquetes` y `Sinónimos` (medir tiempo y contar `[role=treeitem]` ≤ 120); flechas ↓ → ← Home End con `Input.dispatchKeyEvent`; `Ctrl+F12` sobre `AS_PDF3_MOD` → 2 pestañas (spec y body); `axe-cdp.mjs --views objects` en 0.
- [ ] Compuerta global.

---

## Fase F4 — Conexiones y entorno

**Criterio:** guardar una conexión con etiqueta y color; se ve en el chip, la lista y la barra de estado; una conexión vieja sigue abriendo.

### T4.1 Backend: `Environment`
**Archivos:** `crates/sqlt-app/src/store.rs`, tests en el mismo archivo.
```rust
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)] #[serde(rename_all = "lowercase")]
pub enum EnvColor { Neutral, Cyan, Green, Amber, Red, Violet }
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Environment { pub label: String, pub color: EnvColor }
// en ConnectionConfig:  #[serde(default)] pub environment: Option<Environment>
impl Environment { pub fn validated(label: &str, color: EnvColor) -> Result<Self, StoreError> } // trim, 1..=12 chars
```
- [ ] Tests primero: `legacy_json_without_environment_loads` (Review Focus 2: JSON viejo sin el campo → `None`); `environment_round_trips`; `label_is_trimmed_and_limited_to_12`; `empty_label_is_rejected`; `unknown_color_is_rejected`. Verlos fallar, implementar, `cargo test -p sqlt-app` en verde.

### T4.2 Lógica y contraste en TS
**Archivos:** crear `web/src/environment.ts`, `web/src/environment.test.ts`; tokens `--env-<color>-bg` / `--env-<color>-fg` (6 colores × 2 temas) en `styles.css`.
```ts
export const ENV_COLORS = ["neutral","cyan","green","amber","red","violet"] as const;
export type EnvColor = typeof ENV_COLORS[number];
export interface Environment { label: string; color: EnvColor }
validateEnvironment(input: { label: string; color: string }): { ok: true; value: Environment } | { ok: false; error: string }
```
- [ ] Tests primero: validación (recorte, 12 caracteres, color desconocido); y `every env color pair has contrast >= 4.5 in light and dark`, leyendo `styles.css`, extrayendo los hex de los tokens `--env-*` y calculando contraste WCAG (Review Focus 3).
- [ ] Definir los 12 pares de tokens hasta que el test pase.

### T4.3 `ConnectionsView` + `useConnections`
**Archivos:** crear `components/ConnectionsView.tsx`, `web/src/useConnections.ts`; `App.tsx` (se mueve el formulario de conexión actual a esta vista); `api.ts` / `types.ts` (campo `environment`).
Lista con insignia, Conectar / Editar / Eliminar, "Nueva conexión"; el formulario existente más etiqueta (máx. 12) y 6 muestras de color como grupo de radios con `aria-label`.
- [ ] Implementar; `build` → 0.

### T4.4 Insignia en chip, lista, árbol y estado
**Archivos:** `TitleBar.tsx`, `StatusBar.tsx`, `ObjectTree.tsx`, `ConnectionsView.tsx`.
- [ ] Componente `EnvBadge({ environment })` único, usado en los cuatro sitios.
- [ ] CDP (`e2e/env.mjs`): guardar `desa02` con etiqueta `DESA` + cian → aparece en chip, lista, raíz del árbol y barra de estado; abrir una conexión **vieja** sin entorno → conecta y no muestra insignia; `axe-cdp.mjs --views connections` en 0.
- [ ] Compuerta global.

---

## Fase F5 — Historial, estadísticas, paleta anclada, acciones y migas

**Criterio:** pestañas inferiores en 4; la paleta ejecuta los 226 comandos; historial y estadísticas muestran los mismos datos que hoy.

### T5.1 `HistoryView` y `StatsView`
**Archivos:** crear `components/{HistoryView,StatsView}.tsx`; `ResultsPanel.tsx` (pestañas Resultados, Plan, Objeto, Salida); `App.tsx`.
- [ ] Mover el contenido actual de `panel === "history"` y `panel === "stats"` a vistas laterales (mismos datos de `api.history()` / `api.sessionStats`); retirar esas dos pestañas inferiores.
- [ ] CDP: ambas vistas muestran las mismas filas que antes (comparar contra el golden de F4).

### T5.2 Paleta anclada en la barra de título
**Archivos:** `TitleBar.tsx`, `App.tsx` (lógica de `paletteOpen`, `filteredCommands`, `onPaletteKey` se reutiliza), `styles.css`.
Patrón `combobox` + `listbox` (`role="combobox"`, `aria-expanded`, `aria-controls`, `aria-activedescendant`); `Ctrl+Shift+P` enfoca el campo; ↑ ↓ navegan, `Enter` ejecuta, `Esc` cierra. Se retira el modal.
- [ ] CDP: abrir con `Ctrl+Shift+P`, escribir `form`, ejecutar `Edit.FormatSQL`; recorrer una muestra de ≥ 10 comandos de familias distintas y confirmar efecto; `axe` en 0.

### T5.3 `ActionStrip` y `Breadcrumbs`
**Archivos:** crear `components/{ActionStrip,Breadcrumbs}.tsx`; `App.tsx` (se retira el `.toolbar` de 21 botones y `.topbar`); `View.Toolbar` ahora alterna la `ActionStrip`.
`ActionStrip`: Ejecutar (F5), Sentencia, Cancelar | Formatear, Plan | Commit, Rollback | Solo lectura. `Breadcrumbs`: conexión › esquema › tipo › objeto (o "Script sin título").
- [ ] Implementar; verificar que **cada** botón retirado de la barra vieja existe en la tira o en un menú (lista de control con los 21 `title`), nada se pierde.

### T5.4 Transacción pendiente (verdad de Oracle)
**Archivos:** `crates/sqlt-app/src/catalog.rs` o `session.rs`, `desktop/src/main.rs`, `api.ts`, `StatusBar.tsx`, `ActionStrip.tsx`.
```rust
pub fn transaction_pending(session: &Session, options: QueryOptions) -> Result<bool, String>
// select dbms_transaction.local_transaction_id from dual  → Some(id) => true
```
Se consulta al terminar una ejecución y tras commit/rollback (nunca durante una ejecución en curso, para no competir por la sesión). **Sin contador** (Oracle no lo da); solo indicador.
- [ ] **Test primero** (`#[ignore]`, Oracle): `update` sin commit → `true`; `commit` → `false`; `rollback` → `false` (Review Focus 7). Verlo fallar, implementar, ejecutarlo con salida real.
- [ ] Mostrar "Transacción pendiente" en `StatusBar` y marcar Commit/Rollback en `ActionStrip`. Si en la práctica interfiere con la ejecución, **omitirlo y documentarlo** en vez de dejar algo inestable.

### T5.5 Cierre del goal
- [ ] Capturas de la app **real** (claro/oscuro, vistas Objetos/Conexiones/Historial/Estadísticas, paleta abierta, árbol con paquete abierto) en `docs/screenshots/workbench-*.png`.
- [ ] Actualizar `rust/web/DESIGN.md` y la sección de estado del plan.
- [ ] `axe-cdp.mjs` completo (todas las vistas, ambos temas) en 0; compuerta global; instalador final; `MEM-SAVE`.
- [ ] Recién entonces `opencode_loop_goal_complete` con la evidencia de las cinco fases.

---

## Evidencia por fase

*(Cada fase agrega acá, al cerrarse, los comandos corridos y su salida real, el binario y el commit que recibieron la prueba CDP.)*

### Tarea 0 — cerrada (2026-11-26)

- `npm --prefix rust/web i -D axe-core vitest` → `axe-core ^4.14.1`, `vitest ^5.1.2`. En esta red `npm` falla con `SELF_SIGNED_CERT_IN_CHAIN` (proxy con inspección SSL); se resuelve con `NODE_OPTIONS=--use-system-ca` (la verificación SSL sigue activa).
- `npm --prefix rust/web test` → `No test files found, exiting with code 0` (`--passWithNoTests` hasta que existan tests).
- App de la línea base: instalada `%LOCALAPPDATA%\SQLTools\sqlt-desktop.exe` (16.882.592 bytes, 18/11/2026 10:48), código en `rust` al commit `9867f7f`.
- `node rust/web/e2e/flow.mjs --save rust/web/e2e/golden-before.json` → `connected: true`, `chip: "Conectado: anamnesis@desa02"`, `explorer: "list"`, `ddlLoaded: true` (`CREATE TABLE a11 ( tipo_cto NUMBER NULL,`), `tabs: ["SQL","A11 (TABLE)"]`, `gridRows: 1`, `firstRow: "1|X"`. `objectRows` (≈12.6 k) depende de la base y puede variar: comparar con `--ignore objectRows`.
- `node rust/web/e2e/axe-cdp.mjs` (UI actual) → **41 grupos de violaciones** (light: shell 7, results 7, objects 8; dark: shell 6, results 6, objects 7); reglas: `aria-input-field-name`, `aria-required-attr`, `aria-required-parent`, `color-contrast`, `label-title-only`, `nested-interactive`, `page-has-heading-one`, `scrollable-region-focusable`. Detalle en `rust/web/e2e/axe-baseline.txt`.
- **Hallazgos de entorno** (para no repetirlos): (a) la herramienta de shell mata los procesos hijos cuando la llamada termina con código ≠ 0 → la app se lanza **desacoplada** con `rust/web/e2e/launch-app.ps1` (WMI) y los scripts imprimen sus mensajes informativos por stdout; (b) `Ctrl+F12` sobre una tabla puede tardar mucho por el MetaDictionary (`AAA_ELIMINADO`: 17,7 s, `A11`: 0,25 s), por eso el flujo usa el objeto fijo `A11` y espera hasta 60 s; (c) el visor de imágenes cachea por ruta.
- **Hallazgo de producto fuera de alcance (se reporta, no se arregla):** la carga de DDL de tablas vía MetaDictionary puede tardar ~18 s en tablas grandes.

### F1 — cerrada (2026-11-26)

**Qué cambió:** tokens de B (con `--control-border`, `--side`, `--faint`, sintaxis `--t-*`) y `DESIGN.md` reescrito; IBM Plex Sans y JetBrains Mono (subconjunto `latin`, 5 woff2) y licencias en `rust/web/public/licenses/` (viajan en `dist`) + `rust/THIRD_PARTY_NOTICES.md`; `icons.tsx` sobre Phosphor con `IconName` tipado (un nombre sin ícono no compila); `Editor.tsx` con tema 100 % por tokens, `aria-label` y `onCursor`; marco nuevo `TitleBar` / `ActivityBar` / `StatusBar` (en `components/`) con `sideView`; un único `<header>` de aplicación; barra de herramientas conservada (restilada) hasta F5 como `role="toolbar"`; pestañas de documentos como `<ul>` con botones hermanos; `tauri.conf.json` con `minWidth` 1024 y `minHeight` 640.

**Desviaciones respecto del plan:** (1) el rótulo de conexión duplicado de la barra de pestañas se eliminó (la `TitleBar` ya lo muestra); (2) `@types/node` pasó a devDependency porque el `tsc` del build revisa los tests; (3) `npm` falla en esta red por el proxy con inspección SSL: se usa `NODE_OPTIONS=--use-system-ca`; (4) se agregó `e2e/launch-app.ps1`, `e2e/install-local.ps1`, `e2e/shots.mjs`, `e2e/phase-f1.mjs` y `e2e/commands-check.mjs` como herramientas reutilizables de las próximas fases.

**Binario probado:** `%LOCALAPPDATA%\SQLTools\sqlt-desktop.exe`, 17.152.928 bytes, escrito 26/11/2026 11:26, firmado `CN=SQLTools Self-Signed`, construido desde el árbol de trabajo de este commit.

**Evidencia (salida real):**

| Comando | Resultado |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --exclude sqlt-desktop --all-targets -- -D warnings` | exit 0 |
| `cargo check -p sqlt-desktop` | exit 0 |
| `cargo test --workspace --exclude sqlt-desktop` | exit 0, 19 `test result: ok` |
| `npm --prefix rust/web run build` (incluye `tsc --noEmit`) | exit 0 |
| `npm --prefix rust/web test` | **56 passed** (`tokens.test.ts`: contraste de 27 pares × 2 temas; estaba en **rojo con los valores anteriores**: `--muted`/`--surface-sunken` 4,26, `--success` 3,44, `--accent` < 3, `--danger` oscuro) |
| `node e2e/flow.mjs --compare e2e/golden-before.json --ignore objectRows` (app instalada) | `IDENTICAL` (conecta, carga el DDL de `A11` en su pestaña, ejecuta, `1\|X`) |
| `node e2e/axe-cdp.mjs` (instalada) | **`AXE OK: 0 violations in 3 view(s) x 2 themes`** (línea base de la UI anterior: 41 grupos) |
| `node e2e/phase-f1.mjs` (instalada) | `F1 CHECKS OK (11)`: `Ln 3, Col 12` al escribir y `Ln 2, Col 1` tras ↑ + Inicio; 27 controles sin recortes y sin scroll de página a **1024×640**; el editor usa `--surface`/`--text` y la fuente mono en claro y oscuro; Plex Sans y JetBrains Mono cargadas |
| `node e2e/commands-check.mjs --save e2e/golden-commands.json` | `total 283, named 226, implemented 213, withShortcut 193`, digest `535249…e2d5`; la paleta abre desde el botón de la barra de título y con `Ctrl+Shift+P` |
| Capturas 1366×768 y 1024×700, claro y oscuro (sin desbordes: `docW == winW`, `docH == winH`) | `docs/screenshots/workbench-f1-*.png` |

**Hallazgos de la revisión de la propia fase:** los controles de edición (`Ctrl+Shift+P`, `Ctrl+F12`) y el flujo de conexión no cambiaron de comportamiento; la barra de 21 botones sigue hasta F5 (por diseño del plan).

#### F1 — ronda de revisión externa (codex gpt-6.2-sol) y correcciones

La revisión de cierre (el `advisor` interno sigue sin cupo) dijo **"F1 todavía no la daría por cerrada"** y marcó cuatro bloqueos, todos verificados y corregidos:

| Hallazgo | Corrección | Prueba |
|---|---|---|
| Foco al cambiar de vista: `View.ObjectTreeSetFocus` y `NextPane/PrevPane` sin destino con Conexiones abierta; guardar desmonta el control enfocado | `focusObjectsFilter()` cambia a Objetos y enfoca; si el foco se pierde al desmontarse (queda en `<body>`) cae en el botón de la vista; `View.ObjectList/ObjectTree` también muestran Objetos | `phase-f1`: Alt+Shift+1 desde Conexiones → foco en `Filtro`; con foco dentro del formulario, Alt+1 → foco en el botón `Objetos` (no en `BODY`) |
| Conexiones con dos estados que divergen (`<details>` colapsable + vista activa) y un `save/connect` tardío que cierra la vista | Conexiones es una `<section>` con encabezado (sin `<details>`); `connectionsVisit` hace que una acción asíncrona solo cierre la vista de la visita que la inició; se eliminó el shim `setShowConnections` | `phase-f1`: encabezado `#connections-title` y sin `summary` de Conexiones |
| `Ln/Col` dentro de una región viva atómica | `role="status"` solo en el mensaje; `Ln/Col` es texto consultable | `phase-f1`: un único `[role=status]` y no contiene `Ln` |
| `--control-border` 2,82:1 contra `--bg` y 2,67:1 contra `--side` (el test solo lo comparaba con `--surface`) | claro `#8a93a2` → `#7a8394`; el test compara contra `--surface`, `--bg`, `--side` y `--surface-alt` | `tokens.test.ts`: 62 tests (estaba en rojo con el valor anterior) |

Menores incorporados: `aria-pressed` solo en los dos botones que son vistas (Historial y Estadísticas son acciones); la barra de 21 botones es `role="group"` (no promete navegación con flechas); el anillo de foco del editor estaba anulado por una regla vieja `.cm-editor.cm-focused { outline: none }` y ahora se ve; `axe-cdp.mjs` **falla** si no llegó a la vista o al tema pedido y muestra los `incomplete`.

**Hallazgo adicional propio:** al arrancar, la app abría siempre el panel de Conexiones aunque hubiera conexiones guardadas (el efecto `connections.length === 0` corría antes de que cargara la lista; comportamiento heredado que el test de foco destapó). Ahora solo se abre sola si, **ya cargada** la lista, no hay ninguna.

**Evidencia final de F1 (app instalada 26/11/2026 11:39, firmada `CN=SQLTools Self-Signed`):**

| Comando | Resultado |
|---|---|
| `node e2e/phase-f1.mjs` | `F1 CHECKS OK (19)` |
| `node e2e/axe-cdp.mjs` | `AXE OK: 0 violations in 3 view(s) x 2 themes` (con `reached=true` y tema aplicado verificados) |
| `node e2e/keyboard-walk.mjs --theme light` / `dark` | `KEYBOARD WALK OK (35 stops)` en ambos; orden: título → barra → actividad → panel lateral → editor → panel inferior; foco visible en todas las paradas |
| `node e2e/commands-check.mjs --compare e2e/golden-commands.json` | `COMMANDS CHECK OK` (283 / 226 / 213, mismo digest) |
| `node e2e/flow.mjs --compare e2e/golden-before.json --ignore objectRows` | `IDENTICAL` |
| `cargo fmt` / `clippy -D warnings` / `check -p sqlt-desktop` / `test` | exit 0 / 0 / 0 / 0 (19 `test result: ok`) |
| `npm --prefix rust/web run build` / `npm --prefix rust/web test` | exit 0 / **62 passed** |

Golden para F2: `rust/web/e2e/golden-f1.json`.

**No cubierto en F1 (declarado):** lector de pantalla real (NVDA); el recorrido de teclado verifica foco e indicador, no lo que se anuncia. La verificación NVDA queda para el cierre del goal si el entorno lo permite.

### F2 — cerrada (2026-11-26)

Extraídos `components/ResultsPanel.tsx` (339 líneas de JSX fuera de `App.tsx`: 3598 → ~3250), `components/EditorTabs.tsx` y `results.ts` (tipos/constantes/`cellText`). **Desviación declarada:** `SidePanel` no se extrae: el panel lateral usa 34 nombres de `App` y F3 (`ObjectTree`) y F4 (`ConnectionsView`) lo reemplazan por componentes nuevos con su propio hook; extraerlo ahora sería trabajo descartado.

Evidencia (app instalada 26/11/2026 11:46, firmada autofirmada): `flow --compare golden-f1.json` IDENTICAL; `phase-f2.mjs` OK (3: el texto, las pestañas y el historial de deshacer sobreviven al cambio de vista); `phase-f1.mjs` OK; axe 0; keyboard-walk OK; `npm run build` exit 0. Sin cambios de Rust. Revisión externa omitida en F2 por ser extracción mecánica con golden idéntico; se cubre en la revisión de F3.
