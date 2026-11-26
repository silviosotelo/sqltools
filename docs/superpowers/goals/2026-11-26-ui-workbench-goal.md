# Goal: UI workbench completa (propuesta B), fases F1 a F5

Spec: `docs/superpowers/specs/2026-11-26-ui-workbench-design.md` (aprobada por el dueño).
Referencia visual: `docs/ui-proposals/b-workbench/` (maqueta y capturas).

## Objetivo

Reemplazar la interfaz de `rust/web` por la propuesta B en la app real, ejecutando las cinco fases en orden y sin pedir confirmación entre ellas:

1. **F1** — Tokens, fuentes (IBM Plex Sans, JetBrains Mono) e iconos Phosphor con sus licencias; marco nuevo (`TitleBar`, `ActivityBar` con 4 vistas + Atajos/Ajustes, `SidePanel`, `StatusBar`) con el contenido actual adentro.
2. **F2** — Extraer `EditorTabs`, `ResultsPanel` y `SidePanel` de `App.tsx`, sin cambiar comportamiento.
3. **F3** — `ObjectTree` + `useObjectTree` + comando `count_objects`: contadores al conectar, carga por grupo, lista virtualizada, navegación por teclado.
4. **F4** — `ConnectionsView` + `useConnections` + campo `environment` (etiqueta libre + color de paleta fija de 6) en backend y formulario, leyendo conexiones viejas sin migración destructiva.
5. **F5** — `HistoryView` y `StatsView` como vistas laterales, paleta anclada en la barra de título, `ActionStrip` y `Breadcrumbs`; las pestañas inferiores quedan en Resultados, Plan, Objeto y Salida.

## Paso 0 (antes de F1)

Invocar `writing-plans` y escribir `docs/superpowers/plans/2026-11-26-ui-workbench.md` con las cinco fases, tareas, archivos exactos y verificación de cada una. Consultar al `advisor` (o `codex exec` si el advisor no está disponible) con el plan antes de empezar a codificar. Commitear el plan.

## Alcance (no negociable)

- **Se omite** lo que la app no tiene: interruptor Autocommit, filtro de filas de resultados, botón Copiar, pista `@` de la paleta. No se inventan funciones.
- El entorno es **solo visual**: el nombre no cambia comportamiento.
- No se toca: lógica de ejecución, editor CodeMirror (salvo tokens), pestañas de documentos, streaming de resultados, los 226 comandos y sus atajos.
- Tema por defecto: se mantiene "sistema".
- Todo texto nuevo por `i18n.tsx` (es/en). Código, identificadores y errores en inglés.

## Cierre de CADA fase (todo, con salida real; sin esto no se pasa a la siguiente)

1. `cargo fmt --all -- --check`, `cargo clippy --workspace --exclude sqlt-desktop --all-targets -- -D warnings`, `cargo test --workspace --exclude sqlt-desktop` y `npm --prefix rust/web run build` en verde; `vitest` en verde desde que exista (F3).
2. Instalador reconstruido e instalado; **CDP en la app instalada** recorre conectar, explorar, ejecutar y cargar DDL (`Ctrl+F12`) sin regresiones.
3. **axe-core inyectado por CDP**: 0 violaciones en cada pantalla nueva de la fase, en tema claro y oscuro; foco visible en todo lo interactivo.
4. 1366×768 y 1024×700 sin desbordes ni controles cortados.
5. Commit y push a `rust`, con la evidencia anotada en el plan. Guardar memoria (`MEM-SAVE`) al cerrar cada fase.

Criterios propios: ver la tabla de la sección 9 de la spec (F3: expandir Paquetes y Sinónimos fluido y flechas funcionando; F4: conexión vieja sigue abriendo; F5: paleta ejecuta los 226 comandos e historial/estadísticas muestran los mismos datos que hoy).

## Reglas de trabajo durante todo el goal

- Método del dueño (AGENTS.md) en cada fase: plan, advisor antes de comprometer enfoque y antes de dar la fase por cerrada, verificación ejecutada, `INTENT/TWINS/SCOPE/MEM-SAVE`.
- Si el `advisor` y `codex exec` no están disponibles por límite de uso: declararlo, no simular la aprobación, y seguir solo si la fase no depende de esa decisión.
- Un problema real fuera de alcance se **reporta, no se arregla**.
- Si aparece algo que solo el dueño puede resolver (decisión, credencial, paso manual): `opencode_loop_goal_blocked` con lo que falta, y detenerse.
- Al terminar F5: capturas de la app real en `docs/screenshots/`, actualizar `rust/web/DESIGN.md`, y cerrar con `opencode_loop_goal_complete` **solo** con evidencia de las cinco fases.
