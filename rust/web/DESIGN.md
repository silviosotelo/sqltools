# SQLTools UI — design tokens

The interface is a three-pane Oracle IDE: object explorer on the left, editor in
the middle, results/output/history/plan/stats at the bottom, with a top toolbar
and a status bar. The visual language follows the classic SQLTools look: compact
12 px type, 1 px borders, square corners, colour reserved for state.

Colour is expressed only through the tokens below; no rule uses a literal hex
value. `styles.css` declares the light values on `:root` and overrides them for
`[data-theme="dark"]`. `App.tsx` resolves the setting (`system` follows
`prefers-color-scheme`) and writes `data-theme` on `<html>`.

## Surfaces

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#f0f0f0` | `#1e1f22` | window background, toolbars |
| `--surface` | `#ffffff` | `#2b2d30` | editor, grids, panels |
| `--surface-alt` | `#f7f7f7` | `#26282b` | alternating rows, hints |
| `--surface-sunken` | `#e6e6e6` | `#222427` | pressed/hover, headers |

## Lines

| Token | Light | Dark | Use |
|---|---|---|---|
| `--line` | `#c8c8c8` | `#3a3d41` | pane separators, grid rules |
| `--line-soft` | `#d8d8d8` | `#33363a` | internal guides, indent marks |
| `--line-strong` | `#a0a0a0` | `#4a4e54` | input borders, focused edges |

## Text

| Token | Light | Dark | Use |
|---|---|---|---|
| `--text` | `#1a1a1a` | `#dfe1e5` | body text |
| `--text-strong` | `#000000` | `#ffffff` | headings, selected labels |
| `--muted` | `#6b6b6b` | `#9aa0a6` | secondary labels, hints |

## Accent and state

| Token | Light | Dark | Use |
|---|---|---|---|
| `--accent` | `#7aa7d8` | `#4a7fc1` | focus ring, active border |
| `--accent-soft` | `#cfe3fb` | `#2e4b73` | selection, active tab |
| `--accent-wash` | `#eef5ff` | `#24344a` | hovered row, drop target |
| `--accent-strong` | `#33445c` | `#8fb2d8` | brand text on soft accent |
| `--success` | `#2e9e45` | `#4cc26a` | connected dot, valid state |
| `--danger` | `#a33` | `#e06c75` | errors |
| `--shadow-soft` | `#0003` | `#0008` | menus, dropdowns |
| `--shadow` | `#0004` | `#000a` | dialogs and the palette |

## Editor

CodeMirror keeps its own theme so syntax colours follow the app theme
(`Editor.tsx`, a `Compartment` reconfigured when the theme changes). The dark
editor uses the same surfaces (`--surface` on `&`, `--surface-alt` on gutters,
`--accent-soft` for the selection) and a dark syntax palette; the light editor
keeps `defaultHighlightStyle`.

## Rules

- New colours go through a token; a literal hex in a rule is a review defect.
- Contrast: body text keeps at least 7:1 on its surface, muted text 4.5:1, and
  accent-on-soft only for non-text decoration.
- Radius stays 0–3 px, borders 1 px, and no gradients except the print guide
  lines.
