/**
 * When a result grid can be edited, and how. No SQL parser: a query is editable only when it is a
 * single-table SELECT, which a conservative lexical check can recognise. Anything else stays read-only
 * on purpose — guessing is worse than saying "not editable".
 */

export interface EditableQuery {
  /** The statement actually run: the user's select list plus the hidden ROWID column. */
  sql: string;
  /** `owner.table` exactly as written, used by the UPDATE. */
  table: string;
}

export const ROWID_COLUMN = "__sqlt_rowid";

const TOKEN = /^[A-Za-z][A-Za-z0-9_$#]*$/;
const QUALIFIED = /^[A-Za-z][A-Za-z0-9_$#]*\.[A-Za-z][A-Za-z0-9_$#]*$/;
const FORBIDDEN = [
  /\bjoin\b/i,
  /\bunion\b/i,
  /\bintersect\b/i,
  /\bminus\b/i,
  /\bgroup\s+by\b/i,
  /\bhaving\b/i,
  /\bconnect\s+by\b/i,
  /\bstart\s+with\b/i,
  /\bmodel\b/i,
  /\bfor\s+update\b/i,
  /\bdistinct\b/i,
];

/** Index of the first top-level occurrence of `keyword` (outside quotes and parentheses). */
function findTopLevel(sql: string, keyword: string): number {
  const needle = keyword.toLowerCase();
  let depth = 0;
  let quote: string | null = null;
  for (let i = 0; i < sql.length; i++) {
    const ch = sql[i];
    if (quote) {
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === "'" || ch === '"') {
      quote = ch;
      continue;
    }
    if (ch === "(") depth++;
    else if (ch === ")") depth--;
    else if (depth === 0 && (i === 0 || /\s/.test(sql[i - 1]))) {
      if (sql.slice(i, i + needle.length).toLowerCase() === needle) {
        const after = sql[i + needle.length] ?? " ";
        if (!/[A-Za-z0-9_$#]/.test(after)) return i;
      }
    }
  }
  return -1;
}

const stripTerminator = (sql: string) => sql.trim().replace(/;\s*$/, "");

/** `select a, b from t where x = 1` -> the rewritten statement, or null when it is not editable. */
export function prepareEditable(sql: string): EditableQuery | null {
  const text = stripTerminator(sql);
  if (!/^select\b/i.test(text)) return null;
  if (text.includes(":")) return null; // the runner does not bind variables
  const from = findTopLevel(text, "from");
  if (from < 0) return null;
  const list = text.slice(text.toLowerCase().indexOf("select") + "select".length, from).trim();
  const rest = text.slice(from + "from".length).trim();
  if (!list || !rest) return null;
  if (/\browid\b/i.test(list)) return null; // adding another one would collide
  // Only plain column references: an expression cannot carry a ROWID, and `a AS b` or `a b` would make the
  // grid edit `b`, which is not a column of the table.
  const plainColumn = /^[A-Za-z"][A-Za-z0-9_$#".]*$/;
  if (list !== "*" && !list.split(",").every((part) => plainColumn.test(part.trim()))) return null;
  // `a AS b` would make the grid edit `b`, which is not a column of the table.
  if (/\bas\b/i.test(list)) return null;
  if (/\bdistinct\b/i.test(list)) return null;
  if (FORBIDDEN.some((pattern) => pattern.test(rest))) return null;

  // The tail is taken by offset, not rebuilt from tokens: splitting on whitespace would collapse spaces
  // inside a string literal and drop the newline that ends a `--` comment.
  const separator = rest.search(/\s/);
  const tableToken = separator < 0 ? rest : rest.slice(0, separator);
  if (!TOKEN.test(tableToken) && !QUALIFIED.test(tableToken)) return null;
  const tail = separator < 0 ? "" : rest.slice(separator).trimStart();
  // After the table only `where ...` or `order by ...` (or nothing): an alias, a second table or a hint
  // would change what the ROWID refers to.
  if (tail !== "" && !/^where\b/i.test(tail) && !/^order\s+by\b/i.test(tail)) return null;

  const selectList = list === "*" ? "t.*" : list;
  return {
    sql: `select ${selectList}, t.ROWID as "${ROWID_COLUMN}" from ${tableToken} t${tail ? ` ${tail}` : ""}`,
    table: tableToken,
  };
}

/** Query By Example: the user's query becomes a subquery and the condition filters it. */
export function queryByExample(sql: string, condition: string): string {
  const body = stripTerminator(sql);
  const orderBy = findTopLevel(body, "order by");
  const inner = (orderBy < 0 ? body : body.slice(0, orderBy)).trim();
  const order = orderBy < 0 ? "" : body.slice(orderBy).trim();
  const where = condition.trim();
  if (!where) return body;
  // The ORDER BY stays inside the inline view: an alias declared in the query is out of scope outside it.
  const view = order ? `${inner} ${order}` : inner;
  return `select * from (${view}) where ${where}`;
}
