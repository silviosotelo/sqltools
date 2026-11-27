/**
 * Lexical helpers for the editor's completion. No SQL parser: they scan tokens (respecting quotes and
 * parentheses) to answer three questions: which tables are in scope, what alias maps to which table, and
 * which statement the cursor belongs to. Anything unusual simply yields fewer suggestions.
 */

export interface TableInScope {
  /** As written (`owner.table` or `table`). */
  table: string;
  /** Lower-case alias, when the query gives one. */
  alias?: string;
}

/** Words that follow a table name but are not an alias. */
const NOT_AN_ALIAS = new Set(
  (
    "where group order having connect start union minus intersect select join on using set values " +
    "and or left right inner outer full cross natural partition sample for fetch offset when then else " +
    "end returning into from as add modify rename drop cascade enable disable"
  ).split(" "),
);

const TABLE_INTRODUCERS = new Set(["from", "join", "update", "into"]);

interface Token {
  text: string;
  lower: string;
  upper: boolean;
}

/** Words and punctuation of a SQL fragment, ignoring string literals and comments. */
export function tokenize(sql: string): Token[] {
  const tokens: Token[] = [];
  let word = "";
  let quote: string | null = null;
  let lineComment = false;
  let blockComment = false;
  const flush = () => {
    if (word) {
      tokens.push({ text: word, lower: word.toLowerCase(), upper: word === word.toUpperCase() });
      word = "";
    }
  };
  for (let i = 0; i < sql.length; i++) {
    const ch = sql[i];
    const next = sql[i + 1];
    if (lineComment) {
      if (ch === "\n") lineComment = false;
      continue;
    }
    if (blockComment) {
      if (ch === "*" && next === "/") {
        blockComment = false;
        i++;
      }
      continue;
    }
    if (quote) {
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === "-" && next === "-") {
      lineComment = true;
      i++;
      continue;
    }
    if (ch === "/" && next === "*") {
      blockComment = true;
      i++;
      continue;
    }
    if (ch === "'" || ch === '"') {
      flush();
      quote = ch;
      continue;
    }
    if (/[A-Za-z0-9_$#]/.test(ch)) {
      word += ch;
      continue;
    }
    flush();
    if (ch === "." || ch === "," || ch === "(" || ch === ")") {
      tokens.push({ text: ch, lower: ch, upper: false });
    }
  }
  flush();
  return tokens;
}

/** Text of the statement that contains `position` (statements are separated by `;`). */
export function statementAt(sql: string, position: number): string {
  const before = sql.slice(0, position);
  const start = before.lastIndexOf(";") + 1;
  const end = sql.indexOf(";", position);
  return sql.slice(start, end < 0 ? undefined : end);
}

/** Tables and aliases of one statement, in the order they appear (comma lists included). */
export function tablesInScope(statement: string): TableInScope[] {
  const tokens = tokenize(statement);
  const found: TableInScope[] = [];
  for (let i = 0; i < tokens.length; i++) {
    if (!TABLE_INTRODUCERS.has(tokens[i].lower)) continue;
    let j = i + 1;
    for (;;) {
      const token = tokens[j];
      // `from (select ...)` is a subquery, not a table reference
      if (!token || token.text === "(" || !/^[A-Za-z]/.test(token.text)) break;
      let name = token.text;
      let consumed = j;
      if (tokens[j + 1]?.text === "." && tokens[j + 2]) {
        name = `${name}.${tokens[j + 2].text}`;
        consumed = j + 2;
      }
      let maybeAlias = tokens[consumed + 1];
      let aliasOffset = 1;
      if (maybeAlias?.lower === "as") {
        maybeAlias = tokens[consumed + 2];
        aliasOffset = 2;
      }
      const alias =
        maybeAlias && /^[A-Za-z]/.test(maybeAlias.text) && !NOT_AN_ALIAS.has(maybeAlias.lower)
          ? maybeAlias.lower
          : undefined;
      found.push(alias ? { table: name, alias } : { table: name });
      const step = alias ? aliasOffset + 1 : 1;
      const after = tokens[consumed + step];
      if (after && after.text === ",") {
        j = consumed + step + 1;
        continue;
      }
      i = consumed + (alias ? aliasOffset : 0);
      break;
    }
  }
  return found;
}

/** Table a qualifier refers to: its alias if it has one, otherwise the table whose name matches. */
export function tableForQualifier(statement: string, qualifier: string): string | null {
  const lower = qualifier.toLowerCase();
  const tables = tablesInScope(statement);
  const byAlias = tables.find((entry) => entry.alias === lower);
  if (byAlias) return byAlias.table;
  const byName = tables.find(
    (entry) => entry.table.split(".").pop()?.toLowerCase() === lower,
  );
  return byName ? byName.table : null;
}

/** Removes duplicates and caps the list, so the popup stays usable. */
export function limitSuggestions<T>(items: T[], max = 100): T[] {
  const seen = new Set<string>();
  const out: T[] = [];
  for (const item of items) {
    const key = JSON.stringify(item);
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(item);
    if (out.length >= max) break;
  }
  return out;
}
