# Relay design system

Status: **proposed starter**. The project has no design sources yet (no CSS, no theme code),
so every value below is a proposal to accept or change, not something read from the code.

Token names are the `ThemeColor` fields of `gpui-component` 0.7.0
(`src/theme/theme_color.rs`), so the implementation maps one to one onto the GPUI theme.
Relay-specific tokens (methods, status codes, variables) map onto its named colors
(`green`, `yellow`, `blue`, `magenta`, `red`, `cyan`).

Dark is the default theme. Light uses the same names.

## Colors

### Surfaces and text

| Token | Dark | Light | Usage |
|-------|------|-------|-------|
| `background` | `#15171b` | `#ffffff` | Document area, editors, response body |
| `sidebar` | `#111316` | `#f6f7f9` | Sidebar, activity rail |
| `title_bar` | `#111316` | `#f6f7f9` | Window title bar |
| `tab_bar` | `#111316` | `#f6f7f9` | Document tab strip |
| `tab` | `#111316` | `#f6f7f9` | Inactive tab |
| `tab_active` | `#15171b` | `#ffffff` | Active tab, joins the document below it |
| `status_bar` | `#111316` | `#f6f7f9` | Bottom status bar |
| `popover` | `#1c1f24` | `#ffffff` | Menus, command palette, dropdowns |
| `input` | `#1c1f24` | `#ffffff` | Input and URL bar fill |
| `muted` | `#1f2329` | `#f1f3f5` | Table headers, code blocks, chips |
| `border` | `#2a2e35` | `#e2e5ea` | Dividers, input borders, table rules |
| `foreground` | `#e6e8eb` | `#1b1e23` | Primary text |
| `muted_foreground` | `#8b929c` | `#626a75` | Secondary text, placeholders, labels |

### Interaction

| Token | Dark | Light | Usage |
|-------|------|-------|-------|
| `primary` | `#5b8cff` | `#2f6bff` | Send button, primary actions, links |
| `primary_foreground` | `#0b1020` | `#ffffff` | Text on `primary` |
| `primary_hover` | `#7aa2ff` | `#1f57e6` | Hover on `primary` |
| `accent` | `#22262d` | `#eef1f5` | Hovered row, hovered tab |
| `accent_foreground` | `#e6e8eb` | `#1b1e23` | Text on `accent` |
| `list_active` | `#5b8cff1f` | `#2f6bff14` | Selected tree row |
| `list_active_border` | `#5b8cff` | `#2f6bff` | Left edge of the selected tree row |
| `selection` | `#5b8cff40` | `#2f6bff33` | Text selection |
| `ring` | `#5b8cff` | `#2f6bff` | Focus ring, 2 px |

### Semantic

| Token | Dark | Light | Usage |
|-------|------|-------|-------|
| `success` | `#3fb970` | `#1e8a4c` | 2xx status, passed tests |
| `info` | `#4fb3d9` | `#1a86b0` | 3xx status, notices |
| `warning` | `#e0a43a` | `#a86b00` | 4xx status, unresolved variables |
| `danger` | `#f05f5f` | `#d23b3b` | 5xx status, failed tests, network errors, destructive actions |

### HTTP methods

Method labels are colored text, never filled badges.

| Method | Token | Dark | Light |
|--------|-------|------|-------|
| GET | `green` | `#3fb970` | `#1e8a4c` |
| POST | `yellow` | `#e0a43a` | `#a86b00` |
| PUT | `blue` | `#5b8cff` | `#2f6bff` |
| PATCH | `magenta` | `#b07cff` | `#7a4ddb` |
| DELETE | `red` | `#f05f5f` | `#d23b3b` |
| HEAD, OPTIONS, custom | `cyan` | `#4fb3d9` | `#1a86b0` |

### Variables

| Element | Rule |
|---------|------|
| Resolved `{{name}}` | `cyan` text on `cyan` at 12% alpha, radius 3 |
| Unresolved `{{name}}` | `warning` text with a dotted `warning` underline |
| Secret value | Masked as `••••••••`, `muted_foreground`; never rendered in clear by default |

## Typography

| Role | Family | Size / line height | Weight |
|------|--------|--------------------|--------|
| UI | `-apple-system, BlinkMacSystemFont, "Segoe UI", "Inter", "Cantarell", "Noto Sans", sans-serif` | 13 / 18 | 400, 500 for labels, 600 for headings |
| UI small | same | 12 / 16 | 400 |
| Section heading | same | 11 / 16, uppercase, 0.04em tracking | 600 |
| Document title | same | 15 / 20 | 600 |
| Code, URL, headers, bodies | `"JetBrains Mono", "SF Mono", "Cascadia Code", "Fira Code", ui-monospace, monospace` | 13 / 20 | 400 |
| Method label | mono | 11 / 16, uppercase | 700 |

## Spacing

4 px base: `2, 4, 6, 8, 12, 16, 24, 32`.

## Radius

| Token | Value | Usage |
|-------|-------|-------|
| `radius-sm` | 4 | Inputs, buttons, chips |
| `radius-md` | 6 | Popovers, menus, command palette |
| `radius-lg` | 8 | Dialogs |

## Shadows

Only floating surfaces cast a shadow. Panels are separated by `border`, never by shadow.

| Token | Dark | Light |
|-------|------|-------|
| `shadow-popover` | `0 8px 24px rgba(0,0,0,.45)` | `0 8px 24px rgba(16,24,40,.12)` |

## Component rules

| Component | Rule |
|-----------|------|
| Title bar | 36 px. Workspace switcher on the left, environment selector on the right. |
| Document tabs | 32 px. Method label, request name, then a `muted_foreground` dot when the tab holds an unsaved draft; the close button replaces the dot on hover. |
| Tree row | 26 px, 8 px indent per level. Method label in a fixed 44 px column. |
| URL bar | 36 px. Method select, URL in mono with variable highlighting, Send button in `primary`. |
| Tables (params, headers) | 28 px rows, a checkbox column for enabling the row, key and value in mono, `muted` header row. |
| Inputs and buttons | 28 px height, `radius-sm`, 1 px `border`. |
| Status bar | 24 px. Keyboard hints in UI small with `kbd` chips on `muted`. |
| Response summary | Status code and text in its semantic color, then time and size in `muted_foreground`. |
| Focus | Every focusable element shows a 2 px `ring` outline; keyboard focus is never hidden. |
| Density | Compact by default; the app is keyboard-first and data-dense. |
