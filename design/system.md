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

| Token | Purpose | Value |
|-------|---------|-------|
| `accent` | Fill: Send, checked boxes, active tab edge, logo | Pending: cobalt `#2B45B8`, emerald `#17704F` or garnet `#8C1D3C` (see `design/bolt/`) |
| `accent-hover`, `accent-pressed` | Fill states | Derived per accent |
| `accent-tint` | Accent text, selection bar, caret, focus ring | Derived per accent |
| `method-get` ... `method-other` | HTTP method labels | Separate tokens, never aliases of status colors |
| `method-*-dim` | Method labels in the tree and inactive tabs | Mixed toward gray, at least 4.5:1 on the selected-row wash |
| `draft` | Unsaved draft marker | Its own color, not the warning color |

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

- The accent.
- Checked checkboxes use the accent fill and are the heaviest element after Send; a neutral
  check may be calmer.
- The focus ring does not follow the cut corners in the artboards (CSS `clip-path` removes
  outlines). In GPUI the ring is drawn as its own path, so the app can trace the cuts.
- Light theme: Bolt has one; Relay's light accent values are not defined yet.
