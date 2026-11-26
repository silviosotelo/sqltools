/* Shared mock data and helpers for the three UI proposals. No dependencies. */
(function () {
  const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const n0 = new Intl.NumberFormat("es-PY");
  const n2 = new Intl.NumberFormat("es-PY", { minimumFractionDigits: 2, maximumFractionDigits: 2 });

  const KEYWORDS = /\b(select|from|where|and|or|not|null|order|by|group|having|join|left|right|inner|outer|on|as|in|is|update|set|delete|insert|into|values|create|or|replace|editionable|package|body|procedure|function|return|is|end|begin|type|record|table|index|of|varchar2|number|date|pls_integer|desc|asc|rownum|commit|rollback|case|when|then|else|declare|cursor|loop|if|elsif)\b/gi;

  function highlight(line) {
    // Tokenise once so keywords inside strings/comments are left alone.
    const out = [];
    const re = /(--.*$|\/\*.*?\*\/|'(?:[^']|'')*'|"[^"]*"|\b\d+(?:[.,]\d+)?\b|[A-Za-z_][A-Za-z0-9_$#]*)/g;
    let last = 0, m;
    while ((m = re.exec(line))) {
      out.push(esc(line.slice(last, m.index)));
      const t = m[0];
      if (t.startsWith("--") || t.startsWith("/*")) out.push(`<span class="t-c">${esc(t)}</span>`);
      else if (t[0] === "'" || t[0] === '"') out.push(`<span class="t-s">${esc(t)}</span>`);
      else if (/^\d/.test(t)) out.push(`<span class="t-n">${esc(t)}</span>`);
      else if (new RegExp(`^(?:${KEYWORDS.source})$`, "i").test(t)) out.push(`<span class="t-k">${esc(t)}</span>`);
      else out.push(esc(t));
      last = m.index + t.length;
    }
    out.push(esc(line.slice(last)));
    return out.join("");
  }

  const SQL_QUERY = [
    "select c.id_cliente, c.nombre, c.fecha_alta, c.saldo, c.estado, c.id_sucursal",
    "  from cliente c",
    " where c.estado = 'A'",
    "   and rownum <= 14",
    " order by c.fecha_alta desc;",
  ];
  const SQL_ERROR = [
    "select c.id_cliente, c.nombre, s.descripcion",
    "  from cliente c",
    "  join sucursales s on s.id_sucursal = c.id_sucursal",
    " where c.estado = 'A';",
  ];
  const SQL_TX = [
    "update cliente",
    "   set estado = 'I'",
    " where id_cliente in (48211, 48207, 48199);",
  ];
  const PKG_SPEC = [
    'CREATE OR REPLACE EDITIONABLE PACKAGE "ANAMNESIS"."AS_PDF3_MOD" is',
    "/**********************************************",
    "** Additional comment by Andreas Weiden:",
    "** The following methods were added for PK_JRXML_REPGEN",
    "**   PR_GOTO_PAGE, PR_GOTO_CURRENT_PAGE, PR_LINE, PR_POLYGON, PR_PATH",
    "**********************************************/",
    "  type tp_settings is record (",
    "    page_width   number,",
    "    page_height  number,",
    "    margin_left  number",
    "  );",
    "  procedure init;",
    "  function  get_version return varchar2;",
    "  procedure pr_goto_page(i_page_nr number);",
    "  procedure pr_line(p_x1 number, p_y1 number, p_x2 number, p_y2 number);",
    "end;",
  ];
  const PKG_BODY = [
    'CREATE OR REPLACE EDITIONABLE PACKAGE BODY "ANAMNESIS"."AS_PDF3_MOD" is',
    "  -- type tHex is table of pls_integer index by varchar2(2);",
    "  g_page_nr  pls_integer := 0;",
    "  g_version  constant varchar2(10) := '3.2';",
    "",
    "  procedure init is",
    "  begin",
    "    g_page_nr := 1;",
    "  end init;",
    "",
    "  function get_version return varchar2 is",
    "  begin",
    "    return g_version;",
    "  end get_version;",
    "",
    "  procedure pr_goto_page(i_page_nr number) is",
    "  begin",
    "    g_page_nr := i_page_nr;",
    "  end pr_goto_page;",
    "end;",
  ];

  const GRID = {
    cols: [
      { name: "ID_CLIENTE", type: "NUMBER(10)", num: true },
      { name: "NOMBRE", type: "VARCHAR2(60)" },
      { name: "FECHA_ALTA", type: "DATE" },
      { name: "SALDO", type: "NUMBER(14,2)", num: true },
      { name: "ESTADO", type: "VARCHAR2(1)" },
      { name: "ID_SUCURSAL", type: "NUMBER(4)", num: true },
    ],
    rows: [
      [48211, "Benítez Giménez, Marta", "17/11/2026", 1284500.5, "A", 12],
      [48207, "Ortiz Duarte, Ramón", "17/11/2026", 0, "A", 3],
      [48199, "Cabrera Romero, Lidia Beatriz", "16/11/2026", 356000, "A", 12],
      [48183, "Servicios Médicos del Este S.A.", "16/11/2026", 9120750.25, "A", 7],
      [48176, "Villalba Ayala, José Luis", "13/11/2026", null, "A", 3],
      [48170, "Fernández Acosta, Rosa", "13/11/2026", 74200, "A", 21],
      [48164, "Policlínica Ñemby", "12/11/2026", 2310000, "A", 7],
      [48158, "Gauto Mendoza, Carlos Alberto", "12/11/2026", 0, "A", 12],
      [48151, "Rolón Báez, Teresa", "11/11/2026", 128750.75, "A", 3],
      [48147, "Laboratorio Chaco S.R.L.", "11/11/2026", 4486300, "A", 21],
      [48139, "Duarte Ocampo, Mirtha", "10/11/2026", null, "A", 12],
      [48132, "Aquino Zárate, Hugo", "10/11/2026", 51000, "A", 7],
      [48128, "Sanabria Peña, Elvira", "09/11/2026", 903400.1, "A", 3],
      [48121, "Centro Odontológico Asunción", "09/11/2026", 1750000, "A", 21],
    ],
  };

  const OBJECTS = [
    ["A1", "TABLE"], ["A11", "TABLE"], ["AA", "PROCEDURE"], ["AAA", "TABLE"], ["AAA_ELIMINADO", "TABLE"],
    ["AA_ACUERDO_DUP_90", "TABLE"], ["AA_ACUERDO_HIS_MES", "TABLE"], ["ACEPTA_SOLICITUD", "TABLE"],
    ["ACTIVAR_NOT_CRED_TRIG", "TRIGGER"], ["ACTIVO_CODIGO", "VIEW"], ["ACTIVO_FIJO", "TABLE"],
    ["ACTIVO_FIJO_JN", "TABLE"], ["ACUERDO", "SEQUENCE"], ["ACUERDO_EXPENSAS", "TABLE"],
    ["ACUERDO_IMPORTAR", "TABLE"], ["ACUERDO_PRESTADOR", "TABLE"], ["ACUERDO_PRESTADOR_HIST", "TABLE"],
    ["AJUSTAR_A_MONTO", "FUNCTION"], ["AS_PDF3_MOD", "PACKAGE"], ["AS_READ_XLSX", "PACKAGE"],
    ["AUTORIZA_CONSULTA_EXTERNA", "PACKAGE"], ["CG$AJUSTE_ART", "PACKAGE"], ["CG$AJUSTE_STOCK", "PACKAGE"],
    ["CLIENTE", "TABLE"], ["CLIENTE_JN", "TABLE"], ["EST_CTA_PRESPROV_ROW", "TYPE"], ["SUCURSALES", "TABLE"],
    ["WSERVER1", "SYNONYM"],
  ];
  const TYPE_ICON = {
    TABLE: "table", VIEW: "eye", PACKAGE: "package", PROCEDURE: "terminal-window", FUNCTION: "function",
    TRIGGER: "lightning", SEQUENCE: "list-numbers", TYPE: "cube", SYNONYM: "link",
  };
  const TYPE_LABEL = {
    "": "Todos", TABLE: "Tablas", VIEW: "Vistas", PACKAGE: "Paquetes", PROCEDURE: "Procedimientos",
    FUNCTION: "Funciones", TRIGGER: "Triggers", SEQUENCE: "Secuencias", TYPE: "Tipos", SYNONYM: "Sinónimos",
  };
  const COUNTS = { "": 12252, TABLE: 2705, VIEW: 412, PACKAGE: 318, PROCEDURE: 1904, FUNCTION: 1221, TRIGGER: 487, SEQUENCE: 233, TYPE: 96, SYNONYM: 4876 };

  const COMMANDS = [
    ["Script.Execute", "Ejecutar", "F5"],
    ["Script.ExecuteCurrent", "Ejecutar la sentencia actual", "Ctrl+Enter"],
    ["Edit.FormatSQL", "Formatear SQL", "Ctrl+K, E"],
    ["Edit.FormatSQLwithSettings", "Formatear con ajustes", "Ctrl+Shift+K, E"],
    ["Script.LoadDDL", "Cargar DDL del objeto", "Ctrl+F12"],
    ["Script.ExplainPlan", "Plan de ejecución", "F10"],
    ["View.ObjectList", "Lista de objetos", "Alt+3"],
    ["Session.Commit", "Confirmar transacción", "Ctrl+Alt+C"],
    ["Help.Help", "Atajos de teclado", "F1"],
  ];

  function gridHTML(opts) {
    opts = opts || {};
    const head = GRID.cols
      .map((c) => `<th scope="col" class="${c.num ? "num" : ""}"><span class="col-name">${c.name}</span><span class="col-type">${c.type}</span></th>`)
      .join("");
    const body = GRID.rows
      .map((r, i) => {
        const cells = r
          .map((v, j) => {
            const c = GRID.cols[j];
            if (v === null) return `<td class="${c.num ? "num " : ""}null">NULL</td>`;
            if (c.num && c.type.includes(",")) return `<td class="num">${n2.format(v)}</td>`;
            if (c.num) return `<td class="num">${j === 0 || j === 5 ? v : n0.format(v)}</td>`;
            return `<td>${esc(v)}</td>`;
          })
          .join("");
        return `<tr><th scope="row" class="rn">${i + 1}</th>${cells}</tr>`;
      })
      .join("");
    return `<table class="grid" aria-label="Resultados de la consulta"><thead><tr><th class="rn" scope="col"><span class="sr-only">Fila</span>#</th>${head}</tr></thead><tbody>${body}</tbody></table>`;
  }

  function editorHTML(lines, opts) {
    opts = opts || {};
    const rows = lines
      .map((l, i) => {
        const n = i + 1;
        const cls = opts.errorLine === n ? " cm-error" : opts.activeLine === n ? " cm-active" : "";
        return `<div class="cm-row${cls}"><span class="cm-ln" aria-hidden="true">${n}</span><span class="cm-line">${highlight(l) || "&nbsp;"}</span></div>`;
      })
      .join("");
    return `<div class="cm" role="textbox" aria-multiline="true" aria-label="Editor SQL" tabindex="0">${rows}</div>`;
  }

  function objectRows(selected, filter) {
    return OBJECTS.filter((o) => !filter || o[1] === filter)
      .map(([name, type]) => `<li class="obj${name === selected ? " is-selected" : ""}" role="option" aria-selected="${name === selected}" data-type="${type}">${window.icon(TYPE_ICON[type], "obj-ic")}<span class="obj-name">${esc(name)}</span><span class="obj-type">${TYPE_LABEL[type].replace(/s$/, "")}</span></li>`)
      .join("");
  }

  const PALETTE_QUERY = "form";
  function paletteRows(q) {
    const rows = COMMANDS.filter((c) => !q || (c[0] + " " + c[1]).toLowerCase().includes(q.toLowerCase()));
    return rows
      .map((c, i) => `<li class="pal-row${i === 0 ? " is-active" : ""}" role="option" aria-selected="${i === 0}"><span class="pal-label">${esc(c[1])}</span><span class="pal-id">${esc(c[0])}</span><kbd>${esc(c[2])}</kbd></li>`)
      .join("");
  }

  const ERROR = {
    code: "ORA-00942",
    text: "la tabla o vista no existe",
    detail: 'ORA-00942: la tabla o vista no existe\nsentencia 1, línea 3, columna 8: "SUCURSALES"',
    line: 3,
  };

  const SHARED_CSS = `
.sr-only{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
.ic{width:16px;height:16px;fill:currentColor;flex:none}
.mock-bar{position:sticky;top:0;z-index:100;display:flex;gap:6px;align-items:center;flex-wrap:wrap;padding:6px 10px;background:#101114;color:#d9dbe0;font:12px/1.2 "IBM Plex Sans",system-ui,sans-serif;border-bottom:1px solid #2a2c31}
.mock-bar b{font-weight:600;margin-right:6px}
.mock-bar button{font:inherit;color:inherit;background:#1b1d22;border:1px solid #33363d;border-radius:6px;padding:4px 9px;cursor:pointer}
.mock-bar button:hover{background:#262930}
.mock-bar button[aria-pressed="true"]{background:#e6e8ec;color:#101114;border-color:#e6e8ec}
.mock-bar button:focus-visible{outline:2px solid #7cc4ff;outline-offset:2px}
.mock-bar .sp{flex:1}
body.shot .mock-bar{display:none}
`;

  function init(cfg) {
    const params = new URLSearchParams(location.search);
    const style = document.createElement("style");
    style.textContent = SHARED_CSS;
    document.head.appendChild(style);
    const theme = params.get("theme") || cfg.theme || "light";
    document.documentElement.setAttribute("data-theme", theme);
    if (params.get("shot")) document.body.classList.add("shot");
    const bar = document.createElement("nav");
    bar.className = "mock-bar";
    bar.setAttribute("aria-label", "Controles de la maqueta");
    bar.innerHTML = `<b>${esc(cfg.title)}</b>` + cfg.states.map(([k, l]) => `<button type="button" data-state="${k}" aria-pressed="false">${esc(l)}</button>`).join("") + `<span class="sp"></span><button type="button" data-theme-btn aria-pressed="false">Tema oscuro</button>`;
    document.body.prepend(bar);
    let current = params.get("state") || cfg.states[0][0];
    function apply() {
      bar.querySelectorAll("[data-state]").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.state === current)));
      const dark = document.documentElement.getAttribute("data-theme") === "dark";
      bar.querySelector("[data-theme-btn]").setAttribute("aria-pressed", String(dark));
      cfg.render(current);
      document.documentElement.setAttribute("data-state", current);
    }
    bar.addEventListener("click", (e) => {
      const s = e.target.closest("[data-state]");
      if (s) { current = s.dataset.state; apply(); }
      if (e.target.closest("[data-theme-btn]")) {
        const dark = document.documentElement.getAttribute("data-theme") === "dark";
        document.documentElement.setAttribute("data-theme", dark ? "light" : "dark");
        apply();
      }
    });
    apply();
  }

  window.MOCK = {
    esc, n0, n2, highlight, init, gridHTML, editorHTML, objectRows, paletteRows,
    SQL_QUERY, SQL_ERROR, SQL_TX, PKG_SPEC, PKG_BODY, GRID, OBJECTS, TYPE_ICON, TYPE_LABEL, COUNTS, COMMANDS, ERROR, PALETTE_QUERY,
    conn: { user: "anamnesis", alias: "desa02", server: "Oracle Database 12c Standard Edition 12.1.0.2.0" },
  };
})();
