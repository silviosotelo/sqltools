import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { autocompletion, type CompletionContext } from "@codemirror/autocomplete";
import { redo, selectAll, undo } from "@codemirror/commands";
import { sql, StandardSQL } from "@codemirror/lang-sql";
import { EditorState, RangeSet, StateEffect, StateField } from "@codemirror/state";
import {
  EditorView,
  GutterMarker,
  gutter,
  keymap,
  type ViewUpdate,
} from "@codemirror/view";
import { basicSetup } from "codemirror";

type Props = {
  initialValue: string;
  tables: string[];
  loadColumns: (table: string) => Promise<string[]>;
  onChange: (value: string) => void;
  onExecute: (sql: string) => void;
};

class BookmarkMarker extends GutterMarker {
  toDOM() {
    const marker = document.createElement("span");
    marker.textContent = "●";
    marker.style.color = "#c0392b";
    marker.style.cursor = "pointer";
    return marker;
  }
}

const bookmarkMarker = new BookmarkMarker();
const toggleBookmark = StateEffect.define<number>();

const bookmarkField = StateField.define<RangeSet<GutterMarker>>({
  create: () => RangeSet.empty,
  update(value, transaction) {
    let next = value.map(transaction.changes);
    for (const effect of transaction.effects) {
      if (effect.is(toggleBookmark)) {
        const line = transaction.state.doc.lineAt(effect.value);
        let exists = false;
        next.between(line.from, line.from, () => {
          exists = true;
          return false;
        });
        if (exists) {
          next = next.update({ filter: (from) => from !== line.from });
        } else {
          next = next.update({ add: [bookmarkMarker.range(line.from)] });
        }
      }
    }
    return next;
  },
});

const bookmarkGutter = gutter({
  class: "cm-bookmark-gutter",
  lineMarker: (view, line) => {
    let exists = false;
    view.state.field(bookmarkField).between(line.from, line.from, () => {
      exists = true;
      return false;
    });
    return exists ? bookmarkMarker : null;
  },
  initialSpacer: () => bookmarkMarker,
  domEventHandlers: {
    mousedown(view, line) {
      const number = view.state.doc.lineAt(line.from).number;
      view.dispatch({ effects: toggleBookmark.of(number) });
      return true;
    },
  },
});

export type EditorHandle = {
  undo: () => void;
  redo: () => void;
  selectAll: () => void;
  copy: () => Promise<void>;
  cut: () => Promise<void>;
  paste: () => Promise<void>;
  upper: () => void;
  lower: () => void;
  comment: () => void;
  uncomment: () => void;
};

const keywords = [
  "SELECT",
  "FROM",
  "WHERE",
  "JOIN",
  "LEFT",
  "RIGHT",
  "INNER",
  "OUTER",
  "GROUP BY",
  "ORDER BY",
  "HAVING",
  "INSERT",
  "UPDATE",
  "DELETE",
  "MERGE",
  "CREATE",
  "ALTER",
  "DROP",
  "TABLE",
  "VIEW",
  "INDEX",
  "PACKAGE",
  "PROCEDURE",
  "FUNCTION",
  "TRIGGER",
  "DECLARE",
  "BEGIN",
  "END",
  "IF",
  "LOOP",
  "CASE",
  "COMMIT",
  "ROLLBACK",
  "EXPLAIN PLAN",
];

function resolveTable(doc: string, qualifier: string): string {
  const pattern =
    /\b(?:from|join|update|into)\s+([A-Za-z0-9_$#"]+(?:\.[A-Za-z0-9_$#"]+)?)(?:\s+(?:as\s+)?([A-Za-z0-9_$#]+))?/gi;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(doc))) {
    const table = match[1];
    const alias = match[2];
    if (alias && alias.toLowerCase() === qualifier.toLowerCase()) return table;
    if (
      table.split(".").pop()?.toLowerCase() === qualifier.toLowerCase()
    ) {
      return table;
    }
  }
  return qualifier;
}

function completionSource(
  getTables: () => string[],
  getColumns: (table: string) => Promise<string[]>,
) {
  return async (context: CompletionContext) => {
    const dot = context.matchBefore(/[A-Za-z0-9_$#"]+\.$/);
    if (dot) {
      const qualifier = dot.text.slice(0, -1);
      const table = resolveTable(
        context.state.doc.toString(),
        qualifier,
      );
      try {
        const columns = await getColumns(table);
        return {
          from: context.pos,
          options: columns.map((label) => ({ label, type: "property" })),
        };
      } catch {
        return null;
      }
    }
    const word = context.matchBefore(/[\w$#]*/);
    if (!word || (word.from === word.to && !context.explicit)) return null;
    return {
      from: word.from,
      options: [
        ...keywords.map((label) => ({ label, type: "keyword" })),
        ...getTables().map((label) => ({ label, type: "class" })),
      ],
    };
  };
}

export const Editor = forwardRef<EditorHandle, Props>(function Editor({
  initialValue,
  tables,
  loadColumns,
  onChange,
  onExecute,
}: Props, ref) {
  const host = useRef<HTMLDivElement | null>(null);
  const view = useRef<EditorView | null>(null);
  const callbacks = useRef({ onChange, onExecute, tables, loadColumns });
  callbacks.current = { onChange, onExecute, tables, loadColumns };

  const transformSelection = (transform: (text: string) => string) => {
    const editor = view.current;
    if (!editor) return;
    const { from, to } = editor.state.selection.main;
    if (from === to) return;
    editor.dispatch({
      changes: { from, to, insert: transform(editor.state.sliceDoc(from, to)) },
    });
  };

  const toggleComments = (comment: boolean) => {
    const editor = view.current;
    if (!editor) return;
    const selection = editor.state.selection.main;
    const start = editor.state.doc.lineAt(selection.from).number;
    const end = editor.state.doc.lineAt(selection.to).number;
    const changes: { from: number; to?: number; insert?: string }[] = [];
    for (let number = start; number <= end; number += 1) {
      const line = editor.state.doc.line(number);
      if (comment) {
        if (!line.text.startsWith("--")) {
          changes.push({ from: line.from, insert: "-- " });
        }
      } else if (line.text.startsWith("-- ")) {
        changes.push({ from: line.from, to: line.from + 3 });
      } else if (line.text.startsWith("--")) {
        changes.push({ from: line.from, to: line.from + 2 });
      }
    }
    if (changes.length) editor.dispatch({ changes });
  };

  useImperativeHandle(ref, () => ({
    undo: () => {
      if (view.current) undo(view.current);
    },
    redo: () => {
      if (view.current) redo(view.current);
    },
    selectAll: () => {
      if (view.current) selectAll(view.current);
    },
    copy: async () => {
      const editor = view.current;
      if (!editor) return;
      const { from, to } = editor.state.selection.main;
      if (from !== to) {
        await navigator.clipboard.writeText(editor.state.sliceDoc(from, to));
      }
    },
    cut: async () => {
      const editor = view.current;
      if (!editor) return;
      const { from, to } = editor.state.selection.main;
      if (from === to) return;
      await navigator.clipboard.writeText(editor.state.sliceDoc(from, to));
      editor.dispatch({ changes: { from, to, insert: "" } });
    },
    paste: async () => {
      const editor = view.current;
      if (!editor) return;
      const text = await navigator.clipboard.readText();
      editor.dispatch(editor.state.replaceSelection(text));
    },
    upper: () => transformSelection((text) => text.toUpperCase()),
    lower: () => transformSelection((text) => text.toLowerCase()),
    comment: () => toggleComments(true),
    uncomment: () => toggleComments(false),
  }), []);

  useEffect(() => {
    if (!host.current) return;
    const editor = new EditorView({
      state: EditorState.create({
        doc: initialValue,
        extensions: [
          basicSetup,
          bookmarkField,
          bookmarkGutter,
          sql({ dialect: StandardSQL, upperCaseKeywords: true }),
          autocompletion({
            override: [
              (context) =>
                completionSource(
                  () => callbacks.current.tables,
                  (table) => callbacks.current.loadColumns(table),
                )(context),
            ],
          }),
          keymap.of([
            {
              key: "Mod-Enter",
              run: (current) => {
                const { from, to } = current.state.selection.main;
                const selected = current.state.sliceDoc(from, to);
                callbacks.current.onExecute(
                  selected || current.state.doc.toString(),
                );
                return true;
              },
            },
          ]),
          EditorView.updateListener.of((update: ViewUpdate) => {
            if (update.docChanged) {
              callbacks.current.onChange(update.state.doc.toString());
            }
          }),
        ],
      }),
      parent: host.current,
    });
    view.current = editor;
    return () => {
      editor.destroy();
      view.current = null;
    };
  }, []);

  useEffect(() => {
    const editor = view.current;
    if (!editor) return;
    if (editor.state.doc.toString() === initialValue) return;
    editor.dispatch({
      changes: {
        from: 0,
        to: editor.state.doc.length,
        insert: initialValue,
      },
    });
  }, [initialValue]);

  return <div className="editor" ref={host} />;
});
