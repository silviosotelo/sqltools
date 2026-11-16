import { useEffect, useRef } from "react";
import { autocompletion, type CompletionContext } from "@codemirror/autocomplete";
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

function completionSource(getTables: () => string[]) {
  return (context: CompletionContext) => {
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

export function Editor({ initialValue, tables, onChange, onExecute }: Props) {
  const host = useRef<HTMLDivElement | null>(null);
  const view = useRef<EditorView | null>(null);
  const callbacks = useRef({ onChange, onExecute, tables });
  callbacks.current = { onChange, onExecute, tables };

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
                completionSource(() => callbacks.current.tables)(context),
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
}
