# Relay design system: Bolt

Relay uses **Bolt**, the design system of DBFlux, with its own accent. Relay and DBFlux are a
family of apps: same structure, typography, shapes and density; one accent per product.

Source: `DESIGN.md` of DBFlux at commit `f0afcc771e8ab3c059d71f85e34d6074d33f48bc`
(`~/dev/personal/Rust/dbflux-worktree/main`). Bolt is copied for now and will move to a shared
library later. Until then, change Bolt values in both products or record the divergence here.

## What Relay takes from Bolt unchanged

| Area | Values |
|------|--------|
| Neutrals (dark) | desk `#050507`, background `#09090B`, panel `#100F13`, raised `#1A181E`, line `#232128`, line-2 `#37333D`, row divider `#18161B` |
| Text (dark) | strong `#F7F4F7`, body `#C6C3CC`, muted `#8E8996` |
| Washes | hover white 4 %, alternating row white 1.2 %, selected row tint 7 %, selected item tint 12 %, text selection tint 25 %, drop target tint 10 %, scrim desk 62 %, island edge white 5 % |
| Typography | Archivo (interface), Archivo Expanded (uppercase labels, 800, 0.14 em), JetBrains Mono (data, URLs, code, keys); base 13 px; the eight text roles of Bolt (title 20, heading 18, body 13, body small 12, label 11, caption 12, code 13, key hint 12) |
| Shapes | 45 degree cuts on controls and surfaces (keycap 4, control 6, input 8, large control 10, overlay 12, card and island 14, modal 18); data (rows, cells, code) stays square; radius 0 |
| Spacing | 4, 8, 12, 16, 24 px plus a 6 px half step; borders 1 and 2 px; focus ring 1.5 px |
| Chrome | title bar 44, document tab row 46 with 30 px tabs, panel header 40, status bar 38 with 26 px chips, tree row 26 with 14 px indent, controls 30 (24 inline, 44 large) |
| Islands | panel fill cut 14 px at top-left and bottom-right, 1 px island edge along the full outline, no shadow, 8 px of desk between islands and at the side edges |
| Motion | 150 ms, `cubic-bezier(.2, .8, .2, 1)` |

The two Bolt rules about the accent hold for any accent: the accent **fill** is never text on
the dark ground, and accent text, icons, caret and focus ring use the accent **tint**.

## What Relay sets itself

Values from `design/bolt/02-cobalt.html`, rendered on the system board `design/system.html`.
Contrast is measured on panel `#100F13`; "selected" is the selected tree item (tint 12 %).

| Token | Value | Usage | Contrast |
|-------|-------|-------|----------|
| `accent` | `#2B45B8` | Send fill, active tab edge, logo | white text 7.9:1 |
| `accent-hover` | `#3451C8` | Primary hover | white text 6.7:1 |
| `accent-pressed` | `#1F3389` | Primary pressed | white text 11.1:1 |
| `accent-tint` | `#9DB1FF` | Accent text, selection bar, caret, focus ring | 9.2:1 |
| `on-accent` | `#FFFFFF` | Text on accent fills | |
| `draft` | `#F28AC9` | Unsaved draft marker only | 8.4:1 |
| `success` | `#7BE0A0` | 2xx status | 11.8:1 |
| `info` | `#6EA8FF` | 3xx status, links | 7.9:1 |
| `warning` | `#FFC23D` | Unresolved variable, environment warning only | 11.9:1 |
| `danger` | `#FF6B5E` | 4xx and 5xx status, errors | 6.8:1 |
| `syntax-key` | `#A9A4B2` | JSON keys | 7.9:1 |
| `syntax-string` | `#7BE0A0` | Strings | |
| `syntax-number` | `#B79CFF` | Numbers, booleans, null | 8.4:1 |
| `syntax-punct` | `#8E8996` | Punctuation | |
| `method-get` / `-dim` | `#6FD3A0` / `#57A27D` | GET | dim 5.1:1 selected |
| `method-post` / `-dim` | `#F2A65A` / `#B98048` | POST | dim 4.7:1 selected |
| `method-put` / `-dim` | `#5FCFD6` / `#4B9FA5` | PUT | dim 5.1:1 selected |
| `method-patch` / `-dim` | `#C3D865` / `#96A650` | PATCH | dim 5.9:1 selected |
| `method-delete` / `-dim` | `#FF8C80` / `#D4766C` | DELETE | dim 4.9:1 selected |
| `method-other` / `-dim` | `#B4AEC0` / `#938E9D` | HEAD, OPTIONS, custom verbs | dim 4.9:1 selected |

Method colors are their own tokens, never aliases of status colors. The `-dim` values are the
full color blended 75 to 80 % over the panel, solid, so AA holds on the selected item.

## Divergences from Bolt

| Change | Bolt | Relay | Why |
|--------|------|-------|-----|
| Accent | byzantine `#702963`, tint `#D48CC8` | cobalt `#2B45B8`, tint `#9DB1FF` | One accent per product in the family |
| Checked checkbox | accent fill | `raised` fill, `strong` check, `line-2` edge | Send stays the only filled accent in the editor |
| Status bar key hints | inside buttons and chips | status bar only | Hierarchy rule 6 |

## Hierarchy rules

These are Relay's additions to Bolt. They apply to every screen.

1. Two focal points per screen: the request line (method, URL, Send) and the response status.
2. Three text levels: strong (titles, selected item, data values), body, muted (metadata,
   labels, counts, descriptions, line numbers).
3. The accent appears only on: the Send fill, the active document tab edge, the selected tree
   row bar, checked checkboxes, the focus ring and caret, and the logo.
4. Semantic color only for state: unsaved draft, unresolved variable, response status,
   environment warning. One meaning per color.
5. Method labels are plain mono text with no box. Dimmed in the tree and inactive tabs, so the
   request name is what the eye reads; full strength in the URL bar.
6. Counts are muted numbers, never chips. Keyboard hints live in the status bar only.
7. Tables have no vertical rules and no header fill; header labels and descriptions are muted.
8. Section labels are small and muted wayfinding, not content.
9. Secondary actions are ghost buttons that fill on hover. Save is the only secondary-styled
   button.
10. Remove any border that spacing or alignment already implies.

## Open points

- `draft` pink echoes DBFlux's byzantine tint; inside Relay it means only "unsaved", but a
  user of both apps may read it as DBFlux's accent.
- 4xx and 5xx share `danger`; the code number tells them apart.
- The focus ring does not follow the cut corners in the artboards (CSS `clip-path` removes
  outlines). In GPUI the ring is drawn as its own path, so the app can trace the cuts.
- Light theme: Bolt has one; Relay's light accent values are not defined yet.
