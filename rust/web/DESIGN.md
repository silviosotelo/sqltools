# SQLTools UI — design tokens

The interface is an Oracle workbench (proposal B, `docs/ui-proposals/b-workbench`): a title bar with
the command palette, an activity bar, a side panel, the editor with its document tabs, a results
panel and a status bar. Type is IBM Plex Sans (UI) and JetBrains Mono (code, ligatures off), 13 px.

Colour is expressed only through the tokens below; no rule uses a literal hex value outside
`:root` and `[data-theme="dark"]` in `styles.css`. `App.tsx` resolves the setting (`system` follows
`prefers-color-scheme`) and writes `data-theme` on `<html>`. The editor reads the same tokens
(`Editor.tsx`), so there is one source of truth for both themes.

`src/tokens.test.ts` reads `styles.css` and fails the build if a pair below drops under its contrast
ratio in either theme.

## Surfaces

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#f3f4f7` | `#0e1013` | window background |
| `--side` | `#eceef2` | `#12151a` | title bar, activity bar, status bar |
| `--surface` | `#ffffff` | `#171a20` | editor, grids, panels |
| `--surface-alt` | `#f7f8fa` | `#1c2027` | alternating rows, hints, active line |
| `--surface-sunken` | `#e3e6eb` | `#0b0d10` | pressed/hover, headers |

## Lines

| Token | Light | Dark | Use |
|---|---|---|---|
| `--line` / `--line-soft` | `#d9dde4` | `#252a32` | pane separators, grid rules |
| `--line-strong` | `#a9b0bc` | `#3e4551` | separators that need weight |
| `--control-border` | `#7a8394` | `#6b7482` | borders of inputs, selects and buttons (≥ 3:1 on `--surface`, `--bg`, `--side` and `--surface-alt`, WCAG 1.4.11) |

## Text

| Token | Light | Dark | Use | Contrast |
|---|---|---|---|---|
| `--text` | `#1a1e26` | `#dde1e8` | body text | ≥ 4.5:1 on every surface |
| `--text-strong` | `#0b0e13` | `#f5f7fa` | headings, selected labels | ≥ 4.5:1 |
| `--muted` | `#4d5666` | `#98a2b2` | secondary labels | ≥ 4.5:1 on surfaces, bg, side, sunken |
| `--faint` | `#586272` | `#8791a1` | line numbers, hints | ≥ 4.5:1 on surfaces, bg, side |

## Accent and state

| Token | Light | Dark | Use |
|---|---|---|---|
| `--accent` | `#0a7285` | `#3cc4dc` | focus ring, active indicator (≥ 3:1) |
| `--accent-ink` | `#ffffff` | `#04222a` | text on `--accent` (≥ 4.5:1) |
| `--accent-soft` / `--accent-wash` | `#d5eef3` | `#12333b` | selection, active tab, hovered row |
| `--accent-text` / `--accent-strong` | `#075565` | `#8fe0ef` | accent used as text (≥ 4.5:1) |
| `--success` | `#17794a` | `#4cc38a` | connected dot, valid state |
| `--warn` / `--warn-soft` | `#8f5200` / `#fbeccb` | `#e3a93e` / `#35290f` | warnings, search matches |
| `--danger` / `--err-soft` | `#b3261e` / `#fbe4e1` | `#f28b82` / `#3a1d1b` | errors |
| `--shadow-soft` / `--shadow` | `#0003` / `#0004` | `#0008` / `#000a` | menus, dialogs |

## Syntax (editor)

| Token | Light | Dark | Use |
|---|---|---|---|
| `--t-k` | `#0a6a80` | `#59c9de` | keywords |
| `--t-s` | `#8a4b00` | `#e9b46a` | strings |
| `--t-n` | `#0f7a5a` | `#7ad8b4` | numbers, types |
| `--t-c` | `#586272` | `#8791a1` | comments |

## Rules

- New colours go through a token; a literal hex in a rule is a review defect.
- Text keeps at least 4.5:1 on its surface, a control boundary at least 3:1.
- Interactive elements show a focus ring (`:focus-visible`, 2 px `--accent`).
- Radius stays 0–10 px, borders 1 px, no gradients.
- Minimum window size is 1024×640; nothing may clip or scroll the page at that size.
- Environment badges (F4) use `--env-<colour>-bg` / `--env-<colour>-fg`, each pair ≥ 4.5:1 in both themes.

## Structure (workbench)

- **Frame:** `TitleBar` (brand, command palette combobox, connection chip, theme, language) / `ActionStrip` (run, cancel, format, plan, commit, rollback, read-only, connect, overflow menu, breadcrumbs) / `ActivityBar` + side view / editor tabs + editor / bottom tabs / `StatusBar`.
- **Side views** (`sideView`): Objects (`ObjectTree`), Connections (`ConnectionsView`), History (`HistoryView`), Stats (`StatsView`). Shortcuts and Settings are plain actions under the activity bar.
- **Bottom tabs:** Results, Plan, Object, Output (`ResultsPanel`).
- **Object tree:** counts at connect, one lazy `list_objects` per group on expand, virtualised rows of 26 px, WAI-ARIA tree driven by `aria-activedescendant` (the active row is always mounted), `aria-level/setsize/posinset` computed over the filtered tree. Pure logic and its tests live in `src/tree/`.
- **Environment badge:** free label (1..12 characters) + one of 6 palette colors (`neutral, cyan, green, amber, red, violet`), purely visual. `--env-<color>-bg/-fg` tokens (6 x 2 themes, every pair >= 4.5:1, enforced by `environment.test.ts`). One component, `EnvBadge`, used in the chip, the connection list, the objects panel and the status bar.
- **Transaction flag:** `dbms_transaction.local_transaction_id` is asked only when no statement runs (after a run, commit or rollback). It is a flag, never a counter; `FOR UPDATE` is a false positive, `READ ONLY` is not shown and DDL commits implicitly. Unknown on error.
- **Palette:** `role=combobox` + `role=listbox`; `Ctrl+Shift+P` focuses it, arrows move, `Enter` runs, `Esc` closes and returns focus to the editor; the query is emptied on close.
- **Minimum window:** 1024 x 640, no clipped controls and no page scroll (checked by `e2e/phase-f1.mjs`).

## Verification (e2e, against the installed app)

`rust/web/e2e/`: `flow.mjs` (regression flow against a golden), `axe-cdp.mjs` (8 views x 2 themes, must be 0), `keyboard-walk.mjs`, `commands-check.mjs` (283/226/213 digest), `tree.mjs`, `env.mjs`, `phase-f1/f2/f5.mjs`, `shots-final.mjs`. Launch with `launch-app.ps1` (detached), install with `install-local.ps1`.
