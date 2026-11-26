# SQLTools UI — banco de trabajo (propuesta B) · diseño

Fecha: 2026-11-26 · Estado: diseño aprobado por el dueño, spec pendiente de su revisión.

Referencia visual aprobada: `docs/ui-proposals/b-workbench/index.html` (capturas en `docs/ui-proposals/b-workbench/shots/`). La comparación con A y C y los problemas de la UI actual están en `docs/ui-proposals/README.md`.

## 1. Objetivo

Reemplazar la interfaz actual de `rust/web` por la propuesta **B (banco de trabajo moderno)**, en cinco fases, sin romper lo que hoy funciona.

**Éxito.** Al cerrar cada fase la app está usable y probada en la instalada (CDP); `cargo fmt`, `clippy -D warnings`, `cargo test --workspace --exclude sqlt-desktop`, `npm --prefix rust/web run build` y `vitest` pasan; las pantallas nuevas dan **0 violaciones de axe**; los 226 comandos y sus atajos siguen funcionando; la fase queda commiteada.

## 2. Decisiones del dueño

| # | Decisión | Elegido |
|---|---|---|
| 1 | Vistas de la barra de actividad | **Cuatro**: Objetos, Conexiones, Historial, Estadísticas. Las pestañas inferiores quedan en Resultados, Plan de ejecución, Objeto y Salida. Abajo de la barra: Atajos y Ajustes. |
| 2 | Alcance de la insignia de entorno | **Etiqueta libre + color.** Es solo visual: el nombre no cambia el comportamiento (nada de protecciones por "PROD"). |
| 3 | Color del entorno | **Paleta fija de 6** con nombre: neutro, cian, verde, ámbar, rojo, violeta. Cada uno con variante clara y oscura de contraste ≥ 4,5:1. |
| 4 | Carga del árbol | **Contadores con una consulta nueva + carga por grupo** al expandir, con lista virtualizada. |
| 5 | Entrega | **Cinco fases** F1–F5, app usable y verificada al cerrar cada una. |
| 6 | Tema por defecto | **Se mantiene "sistema"** (claro / oscuro / sistema sigue disponible en la barra de título). |
| 7 | Cómo partir `App.tsx` | **Híbrido**: en F2 se extraen solo componentes visuales (estado en `App.tsx`, por props); lo nuevo nace como componente con su propio hook. |

## 3. Alcance honesto

La maqueta muestra cosas que **no son funciones de la app hoy** y se **omiten** (no se inventan; cualquier ampliación sale como pedido aparte):

- Interruptor *Autocommit* (no hay soporte en la app ni en el backend).
- Filtro de filas de resultados.
- Botón *Copiar* de resultados.
- La pista "escribí `@` para objetos" de la paleta.

Sí existen y se conservan: *Solo lectura* (por conexión), *Sentencia* (`Script.ExecuteCurrent`), exportar CSV, formatear, plan, commit y rollback.

Fuera de alcance: cambios en la lógica de ejecución, el editor CodeMirror (salvo tokens de color), las pestañas de documentos (ya hechas), el streaming de resultados y los comandos.

## 4. Frontend

Carpeta nueva `rust/web/src/components/`:

| Componente | Responsabilidad | Fase |
|---|---|---|
| `TitleBar` | Marca, paleta de comandos anclada, chip de conexión con insignia, tema, idioma | F1 (marco), F5 (paleta) |
| `ActivityBar` | Selector de vista lateral + Atajos + Ajustes | F1 |
| `SidePanel` | Aloja la vista activa | F1 |
| `StatusBar` | Conexión, mensaje, transacción pendiente, posición, codificación | F1 |
| `EditorTabs`, `ResultsPanel` | Extracción sin cambio de comportamiento | F2 |
| `ObjectTree` + `useObjectTree` | Árbol conexión › esquema › tipo | F3 |
| `ConnectionsView` + `useConnections` | Lista de conexiones con insignia y formulario | F4 |
| `HistoryView`, `StatsView` | Historial y estadísticas como vistas laterales | F5 |
| `Breadcrumbs`, `ActionStrip` | Migas sobre el editor y tira de acciones | F5 |

Tokens y recursos: los valores de `styles.css` pasan a los de B (acento cian, claro y oscuro; los valores están en el CSS de `docs/ui-proposals/b-workbench/index.html` y se documentan en `rust/web/DESIGN.md` al cerrar F1); IBM Plex Sans y JetBrains Mono por `@fontsource`; iconos Phosphor. Los textos de licencia (OFL 1.1 y MIT) viajan con la app (`docs/ui-proposals/shared/licenses/` es el origen). Todo texto nuevo pasa por `i18n.tsx` (es/en).

## 5. Backend y datos

- **`ConnectionConfig`** gana `environment: Option<Environment>` con `#[serde(default)]`, donde `Environment { label: String, color: EnvColor }`. `label` se recorta y admite 1–12 caracteres; vacío equivale a "sin entorno". `EnvColor` es un enum de 6 valores (`neutral`, `cyan`, `green`, `amber`, `red`, `violet`). Las conexiones ya guardadas se leen igual (sin entorno); no hay migración destructiva.
- **Comando nuevo `count_objects(connection_id, schema)`** → lista de pares `(tipo, cantidad)` desde un único `select object_type, count(*) from all_objects where owner = :1 group by object_type`. Solo se muestran los 9 tipos del explorador; el resto (`PACKAGE BODY`, `INDEX`, …) se ignora en el árbol.
- El árbol usa el `list_objects` existente por tipo al expandir un grupo.

## 6. Estado, errores y casos borde

- **Carga por grupo**: cada grupo es una pequeña máquina de estados (`sin cargar → cargando → listo | error`). Si el usuario cambia de conexión o de esquema a mitad de una carga, el resultado tardío se descarta (mismo criterio que la cancelación actual).
- **Errores**: un fallo de conteo o de carga se muestra **en su grupo** con "Reintentar"; el resto del árbol sigue usable.
- **Lista grande**: filas de 26 px con ventana + sobremuestreo (como la grilla actual); 4.876 sinónimos no cuelgan la interfaz.
- **Filtro del explorador**: filtra los grupos ya cargados; una acción "Buscar en todo el esquema" los carga uno por vez (cancelable). El detalle fino se define en el plan.
- **Sin conexión**: el panel lateral muestra la vista de Conexiones.
- **Ventana chica**: se verifica 1366×768 y 1024×700 (la maqueta solo se midió a 1366); a 1024 la barra de título reduce la paleta.

## 7. Accesibilidad y teclado

- Árbol con roles `tree` / `treeitem` y navegación real con flechas (la maqueta no la implementaba): ↑ ↓ mueven, → expande, ← colapsa, `Enter` abre, `Ctrl+F12` carga DDL como hoy.
- Barra de actividad como botones con `aria-pressed`; las vistas laterales con encabezado propio.
- Foco visible en todo; contraste ≥ 4,5:1 en ambos temas, incluida cada combinación de color de entorno.
- `Ctrl+Shift+P` sigue abriendo la paleta; los 226 comandos y atajos no se tocan.

## 8. Pruebas y verificación

- Se agrega **`vitest` como devDependency** (entorno `node`, sin jsdom) **solo para lógica pura**: aplanado del árbol y ventana virtual, validación de `environment` (etiqueta y color), y el contraste de cada par de color de entorno leyendo los tokens de `styles.css`.
- Rust: tests del comando de conteos (agrupa y descarta tipos no mostrados) y de leer una conexión guardada **sin** el campo `environment`.
- Por fase: CDP contra la app instalada (conectar, explorar, cargar DDL, ejecutar), y `axe-core` inyectado por CDP en cada pantalla nueva (0 violaciones).
- Lo que **no** cubre esta verificación (queda declarado): lector de pantalla, ni rendimiento con esquemas de cientos de miles de objetos.

## 9. Fases y cierre de cada una

Cada fase termina con: compila, `cargo test`/`clippy`/`fmt` y `vitest` en verde, CDP en la instalada, axe en 0 para lo nuevo, commit y push.

| Fase | Contenido | Criterio específico |
|---|---|---|
| **F1** | Tokens, fuentes e iconos de B; marco nuevo (barra de título, barra de actividad, barra de estado) con el contenido actual adentro | La app se ve como B con las funciones de hoy intactas; 1366×768 y 1024×700 sin desbordes |
| **F2** | Extraer `EditorTabs`, `ResultsPanel`, `SidePanel` de `App.tsx` | Cero cambio de comportamiento: mismo recorrido por CDP que antes de la extracción |
| **F3** | `ObjectTree`, `useObjectTree`, `count_objects` | Contadores al conectar; expandir `Paquetes` y `Sinónimos` fluido; `Ctrl+F12` desde el árbol carga spec y body en pestañas; flechas funcionan |
| **F4** | `ConnectionsView`, campo `environment` (backend + formulario) | Guardar una conexión con etiqueta y color; se ve en chip, lista y barra de estado; una conexión vieja sigue abriendo |
| **F5** | `HistoryView`, `StatsView`, paleta en la barra de título, `ActionStrip`, `Breadcrumbs` | Las pestañas inferiores quedan en 4; la paleta ejecuta los 226 comandos; historial y estadísticas muestran los mismos datos que hoy |

## 10. Riesgos

- `App.tsx` (3.533 líneas, 65 `useState`) concentra el riesgo: los handlers dependen unos de otros. Se mitiga extrayendo solo markup en F2 y escribiendo lo nuevo como hooks aislados.
- Integración visual con CodeMirror (colores del editor en ambos temas) al cambiar de tokens.
- El árbol es la primera pieza con navegación por teclado propia; es donde más probable es un defecto de accesibilidad que axe no detecta.
- La maqueta solo se probó a partir de 1366 px de ancho.

## 11. Entregables previstos

Por fase, commits con evidencia en el plan de implementación (`docs/superpowers/plans/`). Al cierre de F5: capturas del resultado real en `docs/screenshots/` y actualización de `rust/web/DESIGN.md`.
