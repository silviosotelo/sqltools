# SQLTools — propuestas de UI/UX

Tres direcciones para rediseñar la interfaz. Son **maquetas estáticas**: no tocan `rust/web` ni la lógica de la app. Cada una tiene seis estados (resultados, paquete con spec y body en pestañas separadas, sin conexión, paleta de comandos, error de ejecución, transacción pendiente), tema claro y oscuro, y la C suma un séptimo (resultados a la derecha).

**Cómo abrirlas** (sin servidor, sin internet; fuentes e iconos van incluidos):

| Propuesta | Archivo |
|---|---|
| A · Clásico refinado | `C:\Users\sotelos\sqltools\docs\ui-proposals\a-clasico\index.html` |
| B · Banco de trabajo moderno | `C:\Users\sotelos\sqltools\docs\ui-proposals\b-workbench\index.html` |
| C · Editor primero | `C:\Users\sotelos\sqltools\docs\ui-proposals\c-editor-primero\index.html` |
| Índice con miniaturas | `C:\Users\sotelos\sqltools\docs\ui-proposals\index.html` |

Arriba de cada maqueta hay una barra oscura para cambiar de estado y de tema. Para fijarlos por URL: `index.html?state=error&theme=dark`; con `&shot=1` se oculta la barra de la maqueta.

## 1. Qué problemas resuelven (sobre la UI actual)

Medidos en las capturas de `docs\screenshots\` y en el código (`App.tsx` tiene 3.533 líneas y 65 `useState`):

1. **Barra superior sin jerarquía**: 21 botones solo-icono sin agrupar ni etiquetas, en dos filas.
2. **Conexión difícil de encontrar**: el formulario largo vive en una sección plegada al fondo del explorador y le roba espacio.
3. **Filtros de tipo**: 9 botones de texto que ocupan 4 filas del explorador.
4. **Lista de objetos en texto plano**, sin iconos por tipo ni contadores.
5. **Tema claro con gris plano** (`#f0f0f0`) y poco contraste entre superficies.
6. **Barra de estado casi vacía**: no informa filas, tiempo, posición, codificación ni transacción.
7. **Sin señal clara de transacción pendiente**, que en Oracle significa cambios sin confirmar.
8. **Estados vacíos y de error pobres** (un texto suelto, sin acción).

Las tres propuestas resuelven los ocho; cambian *cómo*.

## 2. Las tres direcciones

### A · Clásico refinado
- **Producto**: cliente Oracle de escritorio para DBAs y desarrolladores que ya conocen el SQLTools de Windows.
- **Carácter**: familiar, ordenado, silencioso.
- **Mundo**: papel y grafito; la hoja de trabajo de siempre con orden de instrumento.
- **Momentos**: la grilla (tipo de dato en la cabecera, números alineados, NULL tenue) y el explorador con iconos por tipo.
- **Firma**: un punto de estado (verde conectado, ámbar transacción pendiente, rojo error) repetido en la conexión, la barra de estado y la transacción.

Mismo esqueleto de tres paneles. La barra se agrupa en cinco grupos (conexión, ejecución, transacción, herramientas, archivo) con **un solo botón con etiqueta** (Ejecutar F5). Los 9 filtros de tipo pasan a una fila de iconos con contador en el tooltip. El formulario de conexión sale del explorador y pasa a un selector en la barra. Acento azul (el mismo que ya usa la selección actual).

### B · Banco de trabajo moderno
- **Producto**: lo mismo, para quien trabaja con varias bases y esquemas a la vez.
- **Carácter**: concentrado, modular, nocturno.
- **Mundo**: la consola de guardia de un DBA.
- **Momentos**: el árbol conexión › esquema › tipo, y la paleta de comandos como protagonista de la barra superior.
- **Firma**: la **insignia de entorno** (DESA / PROD / LOCAL) junto a la conexión, para no ejecutar nada en producción por error.

Barra de actividad lateral (Objetos, Conexiones, Historial, Estadísticas), árbol real con contadores, migas de pan sobre el editor, acciones de ejecución pegadas al editor y paneles de resultados con filtro de filas. **Oscuro por defecto**; el claro está igual de cuidado. Acento cian.

### C · Editor primero
- **Producto**: lo mismo, pensado para pantallas chicas y para quien vive en el teclado.
- **Carácter**: despejado, directo, sin ruido.
- **Mundo**: cuaderno de laboratorio en neutros puros.
- **Momentos**: el editor a ancho completo con **diagnóstico en línea** (el error aparece bajo la línea que falló) y el panel de resultados acoplable abajo o a la derecha.
- **Firma**: la barra de ejecución pegada al código; el ámbar se usa solo para la acción principal y lo pendiente.

No hay panel lateral permanente: el explorador es un cajón superpuesto (Alt+3), los documentos van en la barra superior, las conexiones en un menú desplegable y la paleta mezcla objetos y comandos. Es la que más espacio da al código.

## 3. Comparación

| | A · Clásico | B · Banco de trabajo | C · Editor primero |
|---|---|---|---|
| Idea | Ordenar lo que ya existe | Estructura tipo IDE moderno | Quitar cromo, priorizar el código |
| Memoria muscular del original | **Alta** | Media | Media |
| Tema por defecto | Claro | Oscuro | Claro |
| Acento | Azul | Cian | Ámbar |
| Explorador | Panel fijo con lista e iconos | Árbol por conexión, esquema y tipo | Cajón superpuesto (Alt+3) |
| Conexiones | Selector en la barra | Vista propia con insignia de entorno | Menú desplegable |
| Acciones de ejecución | Barra superior agrupada | Tira sobre el editor | Barra bajo el editor |
| Resultados | Panel inferior | Panel inferior con filtro de filas | Abajo o a la derecha |
| Error | Bloque en el panel | Bloque en el panel | **En línea** + resumen en el panel |
| Alto de cromo fijo (tabs + barras + estado)¹ | 106 px | 160 px | **94 px** |
| Alto que queda a 768 px para editor + resultados¹ | 662 px | 608 px | **674 px** |
| Riesgo de implementación | **Bajo** | Alto | Medio |

¹ Medido con Playwright a 1366×768 (tema claro, estado `resultados`): cromo = suma de las alturas de las barras fijas; "queda" = editor + divisor de 6 px + panel de resultados. Los encabezados del panel de resultados difieren unos píxeles entre propuestas.

### Pros y contras
- **A**: cambia poco código, no obliga a reaprender nada y resuelve los ocho problemas. A cambio, la estructura queda igual: no gana capacidades nuevas (varias bases, árbol).
- **B**: es lo que más se parece a lo que el usuario ve en otras herramientas modernas y la insignia de entorno agrega seguridad real. Es la que más espacio vertical consume (160 px) y la más cara: hay que partir `App.tsx`.
- **C**: es la que mejor aprovecha una laptop de 768 px y la más rápida de operar con teclado. El explorador oculto cuesta un clic o Alt+3 y los tipos de dato dejan de verse en la cabecera (quedan en tooltip y lectores de pantalla).

## 4. Costo de implementarlo en React

Estimación mía sobre el código actual, **sin medir** tiempos. Líneas de `App.tsx`: barra superior 2330–2572, explorador 2573–2789, zona central (pestañas, editor, panel de resultados, objeto) 2790–3217, barra de estado 3218, diálogos 3230 en adelante. `styles.css` tiene 859 líneas y 128 reglas; los tokens de color ya existen (`DESIGN.md`).

| | Qué se toca | Tamaño |
|---|---|---|
| **A** | Valores de tokens y reglas de barra, explorador, grilla y estado en `styles.css`; reordenar el markup de la barra y el explorador; mover el formulario de conexión a un selector; contenido de la barra de estado. **No cambia estado ni lógica.** | Pequeño |
| **B** | Todo lo anterior más: nuevo marco (barra de actividad + vistas), componente de árbol con teclado y virtualización (hay grupos de miles de objetos), migas de pan, tira de acciones, paleta en la barra superior. Obliga a partir `App.tsx` en componentes. | Grande |
| **C** | Cajón de objetos (reusa la lista actual), barra de pestañas en la barra superior, barra de ejecución, alternancia de ubicación del panel con preferencia guardada, **widget de diagnóstico en línea en `Editor.tsx`** (usa los desplazamientos de `scriptErrors` que ya existen), menú de conexiones. | Mediano |

Cosas que cualquiera de las tres necesita:
- **Fuentes**: IBM Plex Sans y JetBrains Mono (licencia OFL) empaquetadas con `@fontsource`; las maquetas ya las incluyen en `shared\fonts` (5 archivos woff2, 111 KB en total).
- **Iconos**: las maquetas usan Phosphor (licencia MIT). La app tiene hoy su propio `icons.tsx`; unificar implica sumar `@phosphor-icons/react` o portar los iconos usados (el sprite de las maquetas tiene 61).
- **Licencias**: los textos de OFL 1.1 (las dos fuentes) y MIT (Phosphor) ya están en `shared\licenses\`, enumerados en `shared\THIRD_PARTY.md`; tienen que viajar con la app si se implementa.
- **Textos nuevos** por `i18n.tsx` (es/en).

Solo la **insignia de entorno** necesita un cambio en el backend: un campo `entorno` (DESA / PROD / LOCAL) en la configuración de conexión guardada. B y C ya la muestran; en A es una de las dos ideas que se sumarían.

## 5. Recomendación

**Empezar por A y sumarle dos ideas de las otras.**

1. **A como base**: es la única que resuelve los ocho problemas con poco riesgo y sin tirar la memoria muscular de quienes vienen del SQLTools original.
2. **Insignia de entorno (de B)**: el beneficio de seguridad más concreto de las tres propuestas (que `PROD` se vea distinto de `DESA` antes de ejecutar).
3. **Diagnóstico en línea (de C)**: el error aparece donde falló la sentencia; la información ya la tenemos.
4. **B queda para una segunda fase**, si más adelante se quieren varias bases abiertas a la vez: obliga a partir `App.tsx` y conviene hacerlo con el resto del trabajo ya estable.

Si el criterio principal fuera **espacio de pantalla en laptops chicas**, la mejor es **C**.

## 6. Verificación

Corrida real sobre las tres maquetas (`file://`, Chromium, 1366×768 y 1920×1080, claro y oscuro, todos los estados):

```
a-clasico         shots: 16 | page errors: none | axe: 0 critical/serious, 0 moderate/minor | keyboard stops: 40 | without visible focus: []
b-workbench       shots: 16 | page errors: none | axe: 0 critical/serious, 0 moderate/minor | keyboard stops: 39 | without visible focus: []
c-editor-primero  shots: 20 | page errors: none | axe: 0 critical/serious, 0 moderate/minor | keyboard stops: 39 | without visible focus: []
```

- **axe-core**: 0 violaciones en cada propuesta, 2 temas × todos los estados (incluye contraste de color de texto ≥ 4.5:1). Durante el trabajo apareció y se corrigió: pestañas con botones anidados, `role` no permitidos, contraste de textos tenues, foco del editor sin indicador, falta de `h1`, orden de encabezados.
- **Teclado**: se recorre con Tab 40 paradas en el estado de resultados (tema claro) y todas muestran un indicador de foco. El árbol de B no implementa navegación con flechas (es una maqueta).
- Informe crudo: `docs\ui-proposals\verify-report.json`.

**Límites, para no sobrevender esto**:
- Son maquetas: los datos (conteos de objetos, nombres, saldos) son de ejemplo, y casi nada responde al clic salvo el cambio de estado y de tema.
- axe detecta una parte de los problemas de accesibilidad, no todos. **No** se probó con lector de pantalla.
- No se probó por debajo de 1366 px de ancho (A reduce la píldora de comandos a 1280 px, sin verificar).
- Los tiempos de implementación no están medidos.

## 7. Capturas

Carpeta de cada propuesta: `docs\ui-proposals\<propuesta>\shots\`, con nombres `<estado>-<tema>-<ancho>x<alto>.png`. Rutas base:

- `C:\Users\sotelos\sqltools\docs\ui-proposals\a-clasico\shots\`
- `C:\Users\sotelos\sqltools\docs\ui-proposals\b-workbench\shots\`
- `C:\Users\sotelos\sqltools\docs\ui-proposals\c-editor-primero\shots\`

Estados: `resultados`, `paquete`, `sin-conexion`, `paleta`, `error`, `transaccion` (y `dock-derecha` solo en C), cada uno en `light` y `dark` a 1366×768; `resultados`, `paquete` (y `dock-derecha` en C) también a 1920×1080.
