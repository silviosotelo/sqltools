import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { autocompletion, type CompletionContext } from "@codemirror/autocomplete";
import {
  copyLineDown,
  cursorMatchingBracket,
  defaultKeymap,
  deleteCharForward,
  deleteGroupBackward,
  deleteGroupForward,
  deleteLine,
  history,
  historyKeymap,
  indentLess,
  indentMore,
  redo,
  selectAll,
  selectLine,
  selectMatchingBracket,
  undo,
} from "@codemirror/commands";
import {
  closeBrackets,
  closeBracketsKeymap,
  completionKeymap,
  startCompletion,
} from "@codemirror/autocomplete";
import {
  bracketMatching,
  defaultHighlightStyle,
  foldAll,
  foldCode,
  foldEffect,
  foldable,
  foldGutter,
  foldKeymap,
  foldedRanges,
  getIndentUnit,
  HighlightStyle,
  indentOnInput,
  syntaxHighlighting,
  toggleFold,
  unfoldAll,
  unfoldCode,
  unfoldEffect,
} from "@codemirror/language";
import { tags } from "@lezer/highlight";
import {
  findNext,
  findPrevious,
  highlightSelectionMatches,
  openSearchPanel,
  SearchQuery,
  searchKeymap,
  setSearchQuery,
} from "@codemirror/search";
import { sql, StandardSQL } from "@codemirror/lang-sql";
import {
  Compartment,
  EditorState,
  RangeSet,
  StateEffect,
  StateField,
  Transaction,
} from "@codemirror/state";
import {
  Decoration,
  EditorView,
  GutterMarker,
  ViewPlugin,
  WidgetType,
  drawSelection,
  dropCursor,
  gutter,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  rectangularSelection,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";

type Props = {
  initialValue: string;
  tables: string[];
  loadColumns: (table: string) => Promise<string[]>;
  onChange: (value: string) => void;
  onExecute: (sql: string, offset: number | null) => void;
  /** Accessible name of the editing surface. */
  label?: string;
  /** Cursor position (1-based line; column = characters from the start of the line, 1-based, a tab counts as 1). */
  onCursor?: (position: { line: number; col: number }) => void;
  /** Follows the app theme; CodeMirror keeps its own dark palette. */
  theme?: "light" | "dark";
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
const clearBookmarks = StateEffect.define<null>();
const setSlot = StateEffect.define<[number, number]>();

const bookmarkField = StateField.define<RangeSet<GutterMarker>>({
  create: () => RangeSet.empty,
  update(value, transaction) {
    let next = value.map(transaction.changes);
    for (const effect of transaction.effects) {
      if (effect.is(clearBookmarks)) {
        next = RangeSet.empty;
        continue;
      }
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

/// Ten named bookmark slots; positions map through document changes.
const slotField = StateField.define<Map<number, number>>({
  create: () => new Map(),
  update(value, transaction) {
    const next = new Map<number, number>();
    for (const [slot, position] of value) {
      next.set(slot, transaction.changes.mapPos(position));
    }
    for (const effect of transaction.effects) {
      if (effect.is(clearBookmarks)) next.clear();
      if (effect.is(setSlot)) next.set(effect.value[0], effect.value[1]);
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
      view.dispatch({ effects: toggleBookmark.of(line.from) });
      return true;
    },
  },
});

class SpaceMarker extends WidgetType {
  constructor(readonly tab: boolean) {
    super();
  }
  eq(other: SpaceMarker) {
    return other.tab === this.tab;
  }
  toDOM() {
    const marker = document.createElement("span");
    marker.className = this.tab ? "cm-tab" : "cm-space";
    marker.textContent = this.tab ? "→" : "·";
    return marker;
  }
  ignoreEvent() {
    return true;
  }
}

class IndentGuide extends WidgetType {
  toDOM() {
    const guide = document.createElement("span");
    guide.className = "cm-indent-guide";
    return guide;
  }
  ignoreEvent() {
    return true;
  }
}

const spaceMarker = new SpaceMarker(false);
const tabMarker = new SpaceMarker(true);
const guideMarker = new IndentGuide();

function buildWhitespaceDecorations(view: EditorView): DecorationSet {
  const ranges: ReturnType<Decoration["range"]>[] = [];
  for (const { from, to } of view.visibleRanges) {
    const text = view.state.sliceDoc(from, to);
    for (let index = 0; index < text.length; index += 1) {
      const char = text[index];
      if (char === " " || char === "\t") {
        ranges.push(
          Decoration.replace({
            widget: char === "\t" ? tabMarker : spaceMarker,
          }).range(from + index, from + index + 1),
        );
      }
    }
  }
  return Decoration.set(ranges, true);
}

const visibleSpaces = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildWhitespaceDecorations(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged) {
        this.decorations = buildWhitespaceDecorations(update.view);
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

function buildIndentGuides(view: EditorView): DecorationSet {
  const ranges: ReturnType<Decoration["range"]>[] = [];
  const unit = getIndentUnit(view.state);
  for (const { from, to } of view.visibleRanges) {
    let position = from;
    while (position <= to) {
      const line = view.state.doc.lineAt(position);
      let spaces = 0;
      while (spaces < line.text.length && line.text[spaces] === " ") spaces += 1;
      for (let level = unit; level <= spaces; level += unit) {
        ranges.push(
          Decoration.widget({ widget: guideMarker, side: 1 }).range(
            line.from + level,
          ),
        );
      }
      if (line.to + 1 > to) break;
      position = line.to + 1;
    }
  }
  return Decoration.set(ranges, true);
}

const indentGuides = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildIndentGuides(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged) {
        this.decorations = buildIndentGuides(update.view);
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

// One syntax and chrome theme for both app themes: every colour is a CSS token, so the editor
// follows `styles.css` (light and dark) instead of carrying its own hex values.
const tokenHighlightStyle = HighlightStyle.define([
  { tag: tags.comment, color: "var(--t-c)" },
  { tag: tags.keyword, color: "var(--t-k)" },
  { tag: [tags.string, tags.inserted], color: "var(--t-s)" },
  { tag: [tags.number, tags.bool, tags.atom, tags.null], color: "var(--t-n)" },
  { tag: [tags.function(tags.variableName), tags.labelName], color: "var(--accent-text)" },
  { tag: [tags.typeName, tags.className, tags.namespace], color: "var(--t-n)" },
  {
    tag: [tags.operator, tags.operatorKeyword, tags.punctuation, tags.separator],
    color: "var(--muted)",
  },
  { tag: [tags.variableName, tags.propertyName, tags.attributeName], color: "var(--text)" },
  { tag: tags.invalid, color: "var(--danger)" },
]);

const tokenEditorTheme = (dark: boolean) =>
  EditorView.theme(
    {
      "&": { color: "var(--text)", backgroundColor: "var(--surface)" },
      "&.cm-focused": { outline: "2px solid var(--accent)", outlineOffset: "-2px" },
      ".cm-content, .cm-gutters": { fontFamily: "var(--mono)", fontSize: "13px" },
      ".cm-content": { caretColor: "var(--text)" },
      ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--text)" },
      "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
        backgroundColor: "var(--accent-soft)",
      },
      ".cm-activeLine": { backgroundColor: "var(--surface-alt)" },
      ".cm-activeLineGutter": { backgroundColor: "var(--surface-alt)", color: "var(--text)" },
      ".cm-gutters": {
        backgroundColor: "var(--surface)",
        color: "var(--faint)",
        border: "none",
      },
      ".cm-foldPlaceholder": {
        backgroundColor: "var(--surface-sunken)",
        color: "var(--muted)",
        border: "none",
      },
      ".cm-tooltip": {
        backgroundColor: "var(--surface-alt)",
        border: "1px solid var(--line-strong)",
        color: "var(--text)",
      },
      ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
        backgroundColor: "var(--accent-soft)",
        color: "var(--accent-text)",
      },
      ".cm-searchMatch": { backgroundColor: "var(--warn-soft)" },
    },
    { dark },
  );

const themeExtensions = (theme: "light" | "dark") => [
  tokenEditorTheme(theme === "dark"),
  syntaxHighlighting(tokenHighlightStyle),
];

const editorSetup = [
  lineNumbers(),
  highlightActiveLineGutter(),
  highlightSpecialChars(),
  drawSelection(),
  dropCursor(),
  EditorState.allowMultipleSelections.of(true),
  indentOnInput(),
  syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
  bracketMatching(),
  closeBrackets(),
  highlightActiveLine(),
  highlightSelectionMatches(),
  keymap.of([
    ...closeBracketsKeymap,
    // The legacy keymap owns the `Ctrl+K,*` and `Ctrl+Shift+K,*` chords, so the
    // emacs-style bindings `Ctrl-k` (delete to line end) and `Mod-Shift-k`
    // (delete line) would swallow the prefix before the app sees it. Delete
    // line stays on the legacy `Ctrl+Y` (`Edit.DeleteLine`).
    ...defaultKeymap.filter(
      (binding) =>
        binding.key !== "Ctrl-k" &&
        binding.key !== "Mod-Shift-k" &&
        binding.key !== "Shift-Mod-k",
    ),
    ...searchKeymap,
    ...historyKeymap,
    ...foldKeymap,
    ...completionKeymap,
  ]),
];

type DisplayFlags = { spaces: boolean; guides: boolean };

function displayExtensions(flags: DisplayFlags) {
  return [
    ...(flags.spaces ? [visibleSpaces] : []),
    ...(flags.guides ? [indentGuides] : []),
  ];
}

export type EditorHandle = {
  undo: () => void;
  redo: () => void;
  selectAll: () => void;
  copy: () => Promise<void>;
  cut: () => Promise<void>;
  paste: () => Promise<void>;
  upper: () => void;
  lower: () => void;
  invertCase: () => void;
  capitalize: () => void;
  comment: () => void;
  uncomment: () => void;
  selectLine: () => void;
  deleteLine: () => void;
  duplicateLine: () => void;
  joinLines: () => void;
  indent: () => void;
  outdent: () => void;
  find: () => void;
  findNext: () => void;
  findPrevious: () => void;
  foldAll: () => void;
  unfoldAll: () => void;
  trimTrailing: () => void;
  removeBlankLines: () => void;
  sortLines: () => void;
  toggleBookmark: () => void;
  nextBookmark: () => void;
  previousBookmark: () => void;
  setSlotBookmark: (slot: number) => void;
  goToSlotBookmark: (slot: number) => void;
  clearBookmarks: () => void;
  copyBookmarkedLines: () => Promise<void>;
  cutBookmarkedLines: () => Promise<void>;
  deleteBookmarkedLines: () => void;
  tabifyAll: () => void;
  tabifyLeading: () => void;
  untabify: () => void;
  trimLeading: () => void;
  trimExcessive: () => void;
  removeExcessiveBlankLines: () => void;
  splitLine: () => void;
  normalizeKeyword: () => void;
  goToLine: () => void;
  timestamp: () => void;
  textFromCursor: () => string;
  cursorOffset: () => number;
  toggleAllFolds: () => void;
  collapseRegion: () => void;
  expandRegion: () => void;
  toggleRegion: () => void;
  expandSubtree: () => void;
  autocomplete: () => void;
  toggleFolding: () => boolean;
  focus: () => void;
  selectWord: () => void;
  copyAppend: () => Promise<void>;
  cutAppend: () => Promise<void>;
  deleteForward: () => void;
  deleteWordBack: () => void;
  deleteToEndOfWord: () => void;
  scrollUp: () => void;
  scrollDown: () => void;
  scrollToCenter: () => void;
  findSelected: (forward: boolean) => void;
  toggleLineNumbers: () => void;
  matchBracket: () => void;
  selectToMatch: () => void;
  foldToLevel: (level: number) => void;
  toggleSyntaxGutter: () => void;
  toggleRuler: () => void;
  toggleColumnMarkers: () => void;
  toggleVisibleSpaces: () => void;
  toggleIndentGuides: () => void;
  goToOffset: (offset: number) => void;
  goToLineNumber: (line: number) => void;
  setSelectionMode: (mode: "column" | "stream" | "toggle") => boolean;
  justifyColumn: (mode: "left" | "right" | "center") => void;
  insertColumn: () => void;
  insertColumnNumbers: () => void;
  selectedText: () => string;
  insertText: (text: string) => void;
  wordAtCursor: () => string;
};

type DisplayState = {
  flags: DisplayFlags;
  compartment: Compartment;
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
  onCursor,
  label = "SQL",
  theme: themeMode = "light",
}: Props, ref) {
  const host = useRef<HTMLDivElement | null>(null);
  const view = useRef<EditorView | null>(null);
  const display = useRef<DisplayState>({
    flags: { spaces: false, guides: false },
    compartment: new Compartment(),
  });
  const selection = useRef({
    rectangular: false,
    compartment: new Compartment(),
  });
  const theme = useRef({
    mode: themeMode,
    compartment: new Compartment(),
  });
  const historyConf = useRef(new Compartment());
  const folding = useRef({
    enabled: true,
    compartment: new Compartment(),
  });
  const callbacks = useRef({ onChange, onExecute, onCursor, label, tables, loadColumns });
  callbacks.current = { onChange, onExecute, onCursor, label, tables, loadColumns };
  theme.current.mode = themeMode;

  useEffect(() => {
    view.current?.dispatch({
      effects: theme.current.compartment.reconfigure(themeExtensions(themeMode)),
    });
  }, [themeMode]);

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

  const mapLines = (map: (lines: string[]) => string[]) => {
    const editor = view.current;
    if (!editor) return;
    const selection = editor.state.selection.main;
    const wholeDocument = selection.from === selection.to;
    const from = wholeDocument
      ? 0
      : editor.state.doc.lineAt(selection.from).from;
    const to = wholeDocument
      ? editor.state.doc.length
      : editor.state.doc.lineAt(selection.to).to;
    const lines = editor.state.sliceDoc(from, to).split("\n");
    editor.dispatch({ changes: { from, to, insert: map(lines).join("\n") } });
  };

  const bookmarkedLines = () => {
    const editor = view.current;
    if (!editor) return [];
    const result: { from: number; to: number; text: string }[] = [];
    editor.state.field(bookmarkField).between(0, editor.state.doc.length, (from) => {
      const line = editor.state.doc.lineAt(from);
      result.push({ from: line.from, to: line.to, text: line.text });
      return false;
    });
    return result;
  };

  const removeBookmarkedLines = () => {
    const editor = view.current;
    if (!editor) return;
    const marked = new Set(
      bookmarkedLines().map((line) => editor.state.doc.lineAt(line.from).number),
    );
    if (!marked.size) return;
    const kept: string[] = [];
    for (let number = 1; number <= editor.state.doc.lines; number += 1) {
      if (!marked.has(number)) kept.push(editor.state.doc.line(number).text);
    }
    editor.dispatch({
      changes: { from: 0, to: editor.state.doc.length, insert: kept.join("\n") },
      effects: clearBookmarks.of(null),
    });
  };

  const indentWidth = (): number => {
    const editor = view.current;
    return editor ? getIndentUnit(editor.state) : 2;
  };

  const bookmarkPositions = (): number[] => {
    const editor = view.current;
    if (!editor) return [];
    const positions: number[] = [];
    editor.state.field(bookmarkField).between(0, editor.state.doc.length, (from) => {
      positions.push(from);
      return false;
    });
    return positions;
  };

  const jumpToBookmark = (forward: boolean) => {
    const editor = view.current;
    if (!editor) return;
    const positions = bookmarkPositions();
    if (!positions.length) return;
    const head = editor.state.selection.main.head;
    const target = forward
      ? positions.find((position) => position > head) ?? positions[0]
      : [...positions].reverse().find((position) => position < head) ??
        positions[positions.length - 1];
    editor.dispatch({
      selection: { anchor: editor.state.doc.lineAt(target).from },
      effects: EditorView.scrollIntoView(target, { y: "center" }),
    });
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
    invertCase: () =>
      transformSelection((text) =>
        [...text]
          .map((char) =>
            char === char.toLowerCase()
              ? char.toUpperCase()
              : char.toLowerCase(),
          )
          .join(""),
      ),
    capitalize: () =>
      transformSelection((text) =>
        text.replace(/\b\w/g, (char) => char.toUpperCase()),
      ),
    comment: () => toggleComments(true),
    uncomment: () => toggleComments(false),
    selectLine: () => {
      if (view.current) selectLine(view.current);
    },
    deleteLine: () => {
      if (view.current) deleteLine(view.current);
    },
    duplicateLine: () => {
      if (view.current) copyLineDown(view.current);
    },
    joinLines: () =>
      mapLines((lines) => [
        lines
          .map((line) => line.trim())
          .filter(Boolean)
          .join(" "),
      ]),
    indent: () => {
      if (view.current) indentMore(view.current);
    },
    outdent: () => {
      if (view.current) indentLess(view.current);
    },
    find: () => {
      if (view.current) openSearchPanel(view.current);
    },
    findNext: () => {
      if (view.current) findNext(view.current);
    },
    findPrevious: () => {
      if (view.current) findPrevious(view.current);
    },
    foldAll: () => {
      if (view.current) foldAll(view.current);
    },
    unfoldAll: () => {
      if (view.current) unfoldAll(view.current);
    },
    trimTrailing: () =>
      mapLines((lines) => lines.map((line) => line.replace(/\s+$/, ""))),
    removeBlankLines: () =>
      mapLines((lines) => lines.filter((line) => line.trim().length > 0)),
    sortLines: () => mapLines((lines) => [...lines].sort((a, b) => a.localeCompare(b))),
    toggleBookmark: () => {
      const editor = view.current;
      if (!editor) return;
      const line = editor.state.doc.lineAt(editor.state.selection.main.head);
      editor.dispatch({ effects: toggleBookmark.of(line.from) });
    },
    nextBookmark: () => jumpToBookmark(true),
    previousBookmark: () => jumpToBookmark(false),
    setSlotBookmark: (slot) => {
      const editor = view.current;
      if (!editor) return;
      editor.dispatch({
        effects: setSlot.of([slot, editor.state.selection.main.head]),
      });
    },
    goToSlotBookmark: (slot) => {
      const editor = view.current;
      if (!editor) return;
      const position = editor.state.field(slotField).get(slot);
      if (position === undefined) return;
      editor.dispatch({
        selection: { anchor: position },
        effects: EditorView.scrollIntoView(position, { y: "center" }),
      });
    },
    clearBookmarks: () => {
      if (view.current) view.current.dispatch({ effects: clearBookmarks.of(null) });
    },
    copyBookmarkedLines: async () => {
      const lines = bookmarkedLines();
      if (!lines.length) return;
      await navigator.clipboard.writeText(
        lines.map((line) => line.text).join("\n"),
      );
    },
    cutBookmarkedLines: async () => {
      const lines = bookmarkedLines();
      if (!lines.length) return;
      await navigator.clipboard.writeText(
        lines.map((line) => line.text).join("\n"),
      );
      removeBookmarkedLines();
    },
    deleteBookmarkedLines: () => removeBookmarkedLines(),
    tabifyAll: () =>
      mapLines((lines) =>
        lines.map((line) =>
          line.replace(new RegExp(" ".repeat(indentWidth()), "g"), "\t"),
        ),
      ),
    tabifyLeading: () =>
      mapLines((lines) =>
        lines.map((line) => {
          const match = /^ +/.exec(line);
          if (!match) return line;
          const tabs = Math.floor(match[0].length / indentWidth());
          return "\t".repeat(tabs) + line.slice(tabs * indentWidth());
        }),
      ),
    untabify: () =>
      mapLines((lines) =>
        lines.map((line) => line.replace(/\t/g, " ".repeat(indentWidth()))),
      ),
    trimLeading: () => mapLines((lines) => lines.map((line) => line.replace(/^\s+/, ""))),
    trimExcessive: () =>
      mapLines((lines) => lines.map((line) => line.replace(/ {2,}/g, " "))),
    removeExcessiveBlankLines: () =>
      mapLines((lines) =>
        lines.filter(
          (line, index) =>
            !(line.trim() === "" && lines[index - 1]?.trim() === ""),
        ),
      ),
    splitLine: () => {
      const editor = view.current;
      if (!editor) return;
      const head = editor.state.selection.main.head;
      editor.dispatch({
        changes: { from: head, insert: "\n" },
        selection: { anchor: head + 1 },
      });
    },
    normalizeKeyword: () => {
      const editor = view.current;
      if (!editor) return;
      const head = editor.state.selection.main.head;
      const line = editor.state.doc.lineAt(head);
      const offset = head - line.from;
      let start = offset;
      while (start > 0 && /[\w$#]/.test(line.text[start - 1])) start -= 1;
      let end = offset;
      while (end < line.text.length && /[\w$#]/.test(line.text[end])) end += 1;
      if (start === end) return;
      editor.dispatch({
        changes: {
          from: line.from + start,
          to: line.from + end,
          insert: line.text.slice(start, end).toUpperCase(),
        },
      });
    },
    goToLine: () => {
      const editor = view.current;
      if (!editor) return;
      const answer = window.prompt("Line", "1");
      if (!answer) return;
      const number = Math.min(
        Math.max(1, Number(answer) || 1),
        editor.state.doc.lines,
      );
      const line = editor.state.doc.line(number);
      editor.dispatch({
        selection: { anchor: line.from },
        effects: EditorView.scrollIntoView(line.from, { y: "center" }),
      });
    },
    timestamp: () => {
      const editor = view.current;
      if (!editor) return;
      const text = new Date()
        .toISOString()
        .slice(0, 19)
        .replace("T", " ");
      editor.dispatch(editor.state.replaceSelection(text));
    },
    textFromCursor: () => {
      const editor = view.current;
      if (!editor) return "";
      return editor.state.sliceDoc(editor.state.selection.main.head);
    },
    cursorOffset: () => {
      const editor = view.current;
      if (!editor) return 0;
      return editor.state.selection.main.head;
    },
    toggleAllFolds: () => {
      const editor = view.current;
      if (!editor) return;
      if (foldedRanges(editor.state).size) unfoldAll(editor);
      else foldAll(editor);
    },
    collapseRegion: () => {
      if (view.current) foldCode(view.current);
    },
    expandRegion: () => {
      if (view.current) unfoldCode(view.current);
    },
    toggleRegion: () => {
      if (view.current) toggleFold(view.current);
    },
    // `FoldModel::ExpandSubtree`: unfold the foldable region that contains the
    // cursor and every fold nested inside it, by line span.
    expandSubtree: () => {
      const editor = view.current;
      if (!editor) return;
      const head = editor.state.selection.main.head;
      const headLine = editor.state.doc.lineAt(head).number;
      // `FoldModel::FindEnclosingAvailable` works on line numbers, so the anchor
      // line of a region counts as inside it (fold offsets start after the token).
      const containsLine = (range: { from: number; to: number }) => {
        const start = editor.state.doc.lineAt(range.from).number;
        const end = editor.state.doc.lineAt(range.to).number;
        return headLine >= start && headLine <= end;
      };
      const ranges: Array<{ from: number; to: number }> = [];
      foldedRanges(editor.state).between(
        0,
        editor.state.doc.length,
        (from, to) => {
          ranges.push({ from, to });
        },
      );
      let enclosing = ranges
        .filter(containsLine)
        .sort((a, b) => a.to - a.from - (b.to - b.from))[0];
      if (!enclosing) {
        unfoldCode(editor);
        return;
      }
      const effects = ranges
        .filter(
          (range) =>
            range.from >= enclosing.from && range.to <= enclosing.to,
        )
        .map((range) => unfoldEffect.of(range));
      if (effects.length) editor.dispatch({ effects });
    },
    autocomplete: () => {
      if (view.current) startCompletion(view.current);
    },
    toggleFolding: () => {
      const editor = view.current;
      if (!editor) return folding.current.enabled;
      folding.current.enabled = !folding.current.enabled;
      editor.dispatch({
        effects: folding.current.compartment.reconfigure(
          folding.current.enabled ? foldGutter() : [],
        ),
      });
      return folding.current.enabled;
    },
    focus: () => {
      view.current?.focus();
    },
    selectWord: () => {
      const editor = view.current;
      if (!editor) return;
      const head = editor.state.selection.main.head;
      const line = editor.state.doc.lineAt(head);
      const offset = head - line.from;
      let start = offset;
      while (start > 0 && /[\w$#]/.test(line.text[start - 1])) start -= 1;
      let end = offset;
      while (end < line.text.length && /[\w$#]/.test(line.text[end])) end += 1;
      if (start === end) return;
      editor.dispatch({
        selection: { anchor: line.from + start, head: line.from + end },
      });
    },
    copyAppend: async () => {
      const editor = view.current;
      if (!editor) return;
      const { from, to } = editor.state.selection.main;
      if (from === to) return;
      const text = editor.state.sliceDoc(from, to);
      const current = await navigator.clipboard
        .readText()
        .catch(() => "");
      await navigator.clipboard.writeText(
        current ? `${current}\n${text}` : text,
      );
    },
    cutAppend: async () => {
      const editor = view.current;
      if (!editor) return;
      const { from, to } = editor.state.selection.main;
      if (from === to) return;
      const text = editor.state.sliceDoc(from, to);
      const current = await navigator.clipboard
        .readText()
        .catch(() => "");
      await navigator.clipboard.writeText(
        current ? `${current}\n${text}` : text,
      );
      editor.dispatch({ changes: { from, to, insert: "" } });
    },
    deleteForward: () => {
      const editor = view.current;
      if (!editor) return;
      if (editor.state.selection.main.empty) {
        deleteCharForward(editor);
      } else {
        editor.dispatch(editor.state.replaceSelection(""));
      }
    },
    deleteWordBack: () => {
      if (view.current) deleteGroupBackward(view.current);
    },
    deleteToEndOfWord: () => {
      if (view.current) deleteGroupForward(view.current);
    },
    scrollUp: () => {
      const editor = view.current;
      if (!editor) return;
      editor.scrollDOM.scrollTop -= editor.defaultLineHeight * 3;
    },
    scrollDown: () => {
      const editor = view.current;
      if (!editor) return;
      editor.scrollDOM.scrollTop += editor.defaultLineHeight * 3;
    },
    scrollToCenter: () => {
      const editor = view.current;
      if (!editor) return;
      editor.dispatch({
        effects: EditorView.scrollIntoView(
          editor.state.selection.main.head,
          { y: "center" },
        ),
      });
    },
    findSelected: (forward) => {
      const editor = view.current;
      if (!editor) return;
      const { from, to } = editor.state.selection.main;
      if (from === to) return;
      editor.dispatch({
        effects: setSearchQuery.of(
          new SearchQuery({ search: editor.state.sliceDoc(from, to) }),
        ),
      });
      if (forward) findNext(editor);
      else findPrevious(editor);
    },
    toggleLineNumbers: () => {
      const editor = view.current;
      if (!editor) return;
      editor.dom.classList.toggle("hide-line-numbers");
    },
    matchBracket: () => {
      if (view.current) cursorMatchingBracket(view.current);
    },
    selectToMatch: () => {
      if (view.current) selectMatchingBracket(view.current);
    },
    foldToLevel: (level) => {
      const editor = view.current;
      if (!editor) return;
      const ranges: { from: number; to: number }[] = [];
      for (let number = 1; number <= editor.state.doc.lines; number += 1) {
        const line = editor.state.doc.line(number);
        const range = foldable(editor.state, line.from, line.to);
        if (range) ranges.push(range);
      }
      const unique = [
        ...new Map(ranges.map((range) => [range.from, range])).values(),
      ].sort((a, b) => a.from - b.from || b.to - a.to);
      const stack: { to: number }[] = [];
      const effects = [];
      for (const range of unique) {
        while (stack.length && stack[stack.length - 1].to <= range.from) {
          stack.pop();
        }
        if (stack.length < level) effects.push(foldEffect.of(range));
        stack.push({ to: range.to });
      }
      if (effects.length) editor.dispatch({ effects });
    },
    toggleSyntaxGutter: () => {
      const editor = view.current;
      if (!editor) return;
      editor.dom.classList.toggle("hide-fold-gutter");
    },
    toggleRuler: () => {
      const editor = view.current;
      if (!editor) return;
      editor.dom.classList.toggle("show-ruler");
    },
    toggleColumnMarkers: () => {
      const editor = view.current;
      if (!editor) return;
      editor.dom.classList.toggle("show-column-markers");
    },
    toggleVisibleSpaces: () => {
      const editor = view.current;
      if (!editor) return;
      const current = display.current;
      current.flags = { ...current.flags, spaces: !current.flags.spaces };
      editor.dispatch({
        effects: current.compartment.reconfigure(
          displayExtensions(current.flags),
        ),
      });
    },
    toggleIndentGuides: () => {
      const editor = view.current;
      if (!editor) return;
      const current = display.current;
      current.flags = { ...current.flags, guides: !current.flags.guides };
      editor.dispatch({
        effects: current.compartment.reconfigure(
          displayExtensions(current.flags),
        ),
      });
    },
    goToOffset: (offset) => {
      const editor = view.current;
      if (!editor) return;
      const position = Math.max(0, Math.min(offset, editor.state.doc.length));
      editor.dispatch({
        selection: { anchor: position },
        effects: EditorView.scrollIntoView(position, { y: "center" }),
      });
      editor.focus();
    },
    goToLineNumber: (line) => {
      const editor = view.current;
      if (!editor) return;
      const number = Math.max(1, Math.min(line, editor.state.doc.lines));
      const position = editor.state.doc.line(number).from;
      editor.dispatch({
        selection: { anchor: position },
        effects: EditorView.scrollIntoView(position, { y: "center" }),
      });
      editor.focus();
    },
    setSelectionMode: (mode) => {
      const editor = view.current;
      if (!editor) return false;
      const current = selection.current;
      current.rectangular =
        mode === "toggle" ? !current.rectangular : mode === "column";
      editor.dispatch({
        effects: current.compartment.reconfigure(
          current.rectangular ? rectangularSelection() : [],
        ),
      });
      return current.rectangular;
    },
    justifyColumn: (mode) => {
      const editor = view.current;
      if (!editor) return;
      const selection = editor.state.selection.main;
      const start = editor.state.doc.lineAt(selection.from).number;
      const end = editor.state.doc.lineAt(selection.to).number;
      const lines: string[] = [];
      for (let number = start; number <= end; number += 1) {
        lines.push(editor.state.doc.line(number).text);
      }
      const trimmed = lines.map((line) => line.trim());
      const width = Math.max(...trimmed.map((line) => line.length));
      const justified = trimmed.map((line) => {
        const pad = width - line.length;
        if (mode === "right") return " ".repeat(pad) + line;
        if (mode === "center") {
          const left = Math.floor(pad / 2);
          return " ".repeat(left) + line + " ".repeat(pad - left);
        }
        return line;
      });
      editor.dispatch({
        changes: {
          from: editor.state.doc.line(start).from,
          to: editor.state.doc.line(end).to,
          insert: justified.join("\n"),
        },
      });
    },
    insertColumn: () => {
      const editor = view.current;
      if (!editor) return;
      const selection = editor.state.selection.main;
      const start = editor.state.doc.lineAt(selection.from).number;
      const end = editor.state.doc.lineAt(selection.to).number;
      const line = editor.state.doc.lineAt(selection.head);
      const column = selection.head - line.from;
      const changes = [];
      for (let number = start; number <= end; number += 1) {
        const target = editor.state.doc.line(number);
        changes.push({
          from: target.from + Math.min(column, target.length),
          insert: " ",
        });
      }
      editor.dispatch({ changes });
    },
    insertColumnNumbers: () => {
      const editor = view.current;
      if (!editor) return;
      const selection = editor.state.selection.main;
      const start = editor.state.doc.lineAt(selection.from).number;
      const end = editor.state.doc.lineAt(selection.to).number;
      const line = editor.state.doc.lineAt(selection.head);
      const column = selection.head - line.from;
      const changes = [];
      for (let number = start; number <= end; number += 1) {
        const target = editor.state.doc.line(number);
        changes.push({
          from: target.from + Math.min(column, target.length),
          insert: `${number - start + 1} `,
        });
      }
      editor.dispatch({ changes });
    },
    selectedText: () => {
      const editor = view.current;
      if (!editor) return "";
      const { from, to } = editor.state.selection.main;
      return editor.state.sliceDoc(from, to);
    },
    insertText: (text) => {
      const editor = view.current;
      if (!editor) return;
      editor.dispatch(editor.state.replaceSelection(text));
    },
    wordAtCursor: () => {
      const editor = view.current;
      if (!editor) return "";
      const head = editor.state.selection.main.head;
      const line = editor.state.doc.lineAt(head);
      const offset = head - line.from;
      const isWord = (char: string) => /[A-Za-z0-9_$#./\\:-]/.test(char);
      let start = offset;
      while (start > 0 && isWord(line.text[start - 1])) start -= 1;
      let end = offset;
      while (end < line.text.length && isWord(line.text[end])) end += 1;
      return line.text.slice(start, end);
    },
  }), []);

  useEffect(() => {
    if (!host.current) return;
    const editor = new EditorView({
      state: EditorState.create({
        doc: initialValue,
        extensions: [
          editorSetup,
          bookmarkField,
          slotField,
          bookmarkGutter,
          display.current.compartment.of(
            displayExtensions(display.current.flags),
          ),
          selection.current.compartment.of(
            selection.current.rectangular ? rectangularSelection() : [],
          ),
          theme.current.compartment.of(themeExtensions(theme.current.mode)),
          historyConf.current.of(history()),
          folding.current.compartment.of(foldGutter()),
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
                const { from, to, head } = current.state.selection.main;
                const selected = current.state.sliceDoc(from, to);
                callbacks.current.onExecute(
                  selected || current.state.doc.toString(),
                  selected ? null : head,
                );
                return true;
              },
            },
          ]),
          EditorView.contentAttributes.of({ "aria-label": callbacks.current.label, "aria-multiline": "true" }),
          EditorView.updateListener.of((update: ViewUpdate) => {
            if (update.docChanged) {
              callbacks.current.onChange(update.state.doc.toString());
            }
            if (update.docChanged || update.selectionSet) {
              const head = update.state.selection.main.head;
              const line = update.state.doc.lineAt(head);
              callbacks.current.onCursor?.({ line: line.number, col: head - line.from + 1 });
            }
          }),
        ],
      }),
      parent: host.current,
    });
    view.current = editor;
    callbacks.current.onCursor?.({ line: 1, col: 1 });
    return () => {
      editor.destroy();
      view.current = null;
    };
  }, []);

  useEffect(() => {
    const editor = view.current;
    if (!editor) return;
    if (editor.state.doc.toString() === initialValue) return;
    // Switching documents replaces the buffer; it must not land in the undo
    // history and the stack is reset so Ctrl+Z cannot walk into another tab's
    // text (each tab gets its own history from here on).
    editor.dispatch({
      changes: {
        from: 0,
        to: editor.state.doc.length,
        insert: initialValue,
      },
      annotations: Transaction.addToHistory.of(false),
      effects: historyConf.current.reconfigure(history()),
    });
  }, [initialValue]);

  return <div className="editor" ref={host} />;
});
