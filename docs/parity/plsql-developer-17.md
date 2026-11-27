# Parity inventory: PL/SQL Developer 17 vs SQLTools (Rust)

Purpose: a factual checklist of what PL/SQL Developer 17 (Allround Automations) offers, and what this
project already has, so the gaps can be planned and prioritised.

Method and limits:

- Features are taken from the vendor's **public** documentation (product "Features" page, read on
  2026-11-26) and from **observable** facts of the installed copy: preference keys in
  `C:\Program Files\PLSQL Developer 17\Preferences\Default.ini`, the documented plug-in interface
  (`PlugInDoc\plugindoc.pdf`) and the shipped plug-ins.
- Nothing here comes from decompiling or copying PL/SQL Developer's binaries, UI, icons or strings.
  The product is commercial and closed source; implementation is ours.
- "Have" is checked against this repository: the 226-command registry
  (`crates/sqlt-app/src/commands.rs`, digest in `rust/web/e2e/golden-commands.json`), the workbench UI
  and the `sqlt-app`/`sqlt-meta` crates.

## Summary

| | Count |
|---|---|
| Capabilities tracked (inside the vendor's 20 areas) | 23 |
| Complete | 4 |
| Partial | 9 |
| Missing | 10 |

## Areas

| # | Area (vendor) | What it does | Our status | Evidence in this repo |
|---|---|---|---|---|
| 1 | Powerful PL/SQL Editor | Syntax highlighting, SQL/PL/SQL help, object descriptions, Code Assistant, compiler hints, refactoring, beautifier, code contents, code hierarchy, folding, hyperlink navigation, macro library, Unicode | **Partial** | Since P3: a Code Assistant driven by the dictionary — every object type of the owner, columns of the tables in the statement, aliases resolved lexically, 100 suggestions capped. Plus highlighting, folding, bookmarks, 114 `Edit.*` commands, outline, formatter, `OpenFileUnderCursor`. Missing: refactoring, inline compiler hints, macro library, a separate Code Contents window |
| 2 | Integrated Debugger | Step in/over/out, run until exception, breakpoints, variables, call stack, on any program unit without modifying it | **Partial** | Since P6: a real two-session debugger over DBMS_DEBUG — step over/into/out with breakflags, breakpoints via the command, variable watch, and a debug strip with the current position; the target session is closed on stop. Missing: a breakpoint UI, call stack view and run-until-exception |
| 3 | AI Assistant | Predefined and user AI functions to create/modify/fix/explain code, with an API key | **Missing** | — |
| 4 | PL/SQL Beautifier | Rule-based formatting, optional on compile/save/open | **Have** | `Edit.FormatSQL`, `Edit.FormatSQLwithSettings`, formatter profiles (`sqlt-app`), formatted in F1–F5 |
| 5 | SQL Window | Multiple statements, editable grid, QBE search in results, recall from history | **Partial** | Since P2: single-table results are editable by ROWID with optimistic concurrency (`rows_affected = 0` is reported as a conflict), NULL is supported, and a WHERE filter wraps the query (QBE). Plus editor, virtualised grid, history and export CSV. Missing: editing rows of joins/views/aggregates, multi-cell edits |
| 6 | Command Window | SQL*Plus look and feel with built-in script editor | **Partial** | `Script.ExecuteInSQLPlus`, script runner with events, errors from `ALL_ERRORS`. Missing: embedded command window UI |
| 7 | Reports | Built-in and custom reports, report files, command-line runner | **Missing** | — |
| 8 | Diagrams | Graphical selection of objects and relations, usable as workspace | **Missing** | — |
| 9 | Projects | Source files + DB objects + notes + options as a unit; compile all; move between databases | **Partial** | Workspaces (`Workspace.*`, 9 commands) cover save/load of editor buffers only |
| 10 | Regression Testing (Test Manager) | Test sets, run, launch debugger on failure, run from Command Window | **Missing** | — |
| 11 | To-Do Items | Notes inside sources, listed per object or project | **Have** | Since P4: a To-Do side view lists every TODO/FIXME of the schema sources with object and line, and opens the source on click (markers must be upper case, whole words and open a comment, so Spanish prose is not a note) |
| 12 | Object Browser | Configurable tree: descriptions, definitions, test scripts, enable/disable triggers and constraints, recompile invalid, query/edit data, text search in sources, drag and drop, dependencies | **Partial** | Since P1: dependencies in both directions, enable/disable triggers and constraints, recompile one unit or every invalid one, all from the panel and the context menu. Plus `ObjectTree` counts and lazy groups, DDL (`Ctrl+F12`), columns/constraints/indexes/triggers/grants, `View.FindInFiles`, `Tools.ExtractSchemaDDL`. Missing: drag and drop, test scripts, query/edit data (P2) |
| 13 | File Browser | Docked tree over user-defined locations; open/save/rename/delete; VCS actions with the plug-in | **Partial** | File open/save/close (`File.*`), `View.FilePanel`. Missing: docked browser over locations |
| 14 | Connection List | Multiple connections at once, hierarchical, windows per connection | **Have** | `ConnectionsView`, environment badge (F4), per-connection sessions |
| 15 | DBMS Scheduler | Manage `DBMS_SCHEDULER` jobs, programs, windows; query run info | **Missing** | — |
| 16 | Performance Optimizing | PL/SQL Profiler per line; statistics of executed SQL and programs (CPU, I/O, sorts…) | **Partial** | Since P5: a real per-line profiler over DBMS_PROFILER (unit, line, nanoseconds, occurrences, source text) with a Profile-statement action. Plus session statistics. Missing: aggregated runs and SQL-level statistics per statement |
| 17 | Oracle Manuals | PDF/HTML manuals integrated for context help | **Partial** | `Help.Help`, `Help.SqlHelp`, `Help.ContextHelp` |
| 18 | Non-PL/SQL Objects | Forms to create/alter tables, sequences, synonyms, libraries, directories, jobs, queues, users, roles generating the SQL | **Missing** | — (DDL is read-only today) |
| 19 | Template List | Insert standard code, create new program files from templates | **Have** | `templates` module (`save_list_load_delete`), `Edit.CreateTemplate`, `Edit.ExpandTemplate`, `View.Snippets` |
| 20 | Query Builder | Graphical SELECT builder: drag tables, columns, where, order by, FK joins | **Missing** | — |
| 21 | Compare User Objects | Compare object selections, show differences, apply or save a script | **Partial** | `sqlt-meta::compare_ddl` (DDL comparison with first difference). Missing: the compare window and script generation |
| 22 | Export User Objects | Export DDL of a selection of objects | **Have** | `Tools.ExtractSchemaDDL` (writes a DDL file) |
| 23 | Tools | Find database objects, invalid objects, import/export tables, test data generator, text/ODBC import, compare data, `dbms_alert`/`dbms_pipe` monitor, session info | **Partial** | `Script.FindObject`, `Session.RefreshObjectCache`. Missing: the rest |

## Observable facts collected (for interoperability work)

- Preferences are plain INI files (`Preferences\Default.ini`, `Default.ExternalTools.prefs`): sections
  `Colors`, `Fonts`, `Files`, and file-type extensions (`FncExt=fnc`, `PrcExt=prc`, `TrgExt=trg`,
  `PkgExt=pck`, `TypExt=typ`, `SpcExt=spc`, `BdyExt=bdy`, `TpsExt=tps`, `TpbExt=tpb`, `PrjExt=prj`).
  These are readable and could be imported (themes, file associations).
- The plug-in interface is documented (`PlugInDoc\plugindoc.pdf`, C# demo links): a legitimate route for
  building "PLSQL Pro Tools" as a plug-in instead of cloning the IDE.

## Non-goals

- Decompiling or reproducing PL/SQL Developer's implementation, UI, icons or strings.
- Shipping anything that requires its licence or assets.

## Open questions for the owner

1. Which of the nine missing areas matter first: debugger, reports, diagrams, projects, test manager,
   to-do, scheduler, object forms, query builder?
2. Should the AI Assistant be ours (provider-agnostic) or left out?
3. Is "PLSQL Pro Tools" a plug-in for PL/SQL Developer (we would use its documented plug-in interface)
   or a separate application built on this repository?
