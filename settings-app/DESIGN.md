# Settings app: design system

Generated with the **ui-ux-pro-max** skill (github.com/nextlevelbuilder/ui-ux-pro-max-skill, MIT) for
"label printer settings desktop utility", then adapted. Its "Hero + Features + CTA" page pattern is a landing
page and does not apply; the style, palette, type and checklist do.

## Style
Minimalism & Swiss: clean, spacious, functional, high contrast, grid based, flat. Hairline borders, no shadows
except the select popover. Light and dark follow the system setting. Dials used: variance 3, motion 3, density 7.

## Colour (tokens in `ui/styles.css`, never raw hex in components)

| Token | Light | Dark | Use |
|---|---|---|---|
| `--primary` / `--on-primary` | `#1E3A5F` / `#FFFFFF` | `#8DB4E8` / `#0B1220` | primary button, checked box, language switch |
| `--accent` | `#2563EB` | `#7AA7FF` | focus ring, selected borders |
| `--background` / `--surface` | `#F8FAFC` / `#FFFFFF` | `#0B1220` / `#121B2E` | page, cards |
| `--foreground` / `--muted` | `#0F172A` / `#475569` | `#E6EAF2` / `#9AA8BD` | text |
| `--control-border` | `#7C8798` | `#7B8AA3` | boxes and inputs (3:1) |
| success / warning / error | `#047857` / `#92400E` / `#B91C1C` | `#34D399` / `#FBBF24` / `#F87171` | status text on tinted backgrounds |

Dark values are lighter, desaturated tonal variants, not inverted. All text pairs were measured at 4.5:1 or
better and control borders and focus at 3:1 or better, in **both** themes (checked 2026-10-07, all pass).

## Type
One sans-serif, **Work Sans** (Google Fonts, OFL). The skill also suggests Outfit, which has **no Vietnamese
subset** (data/google-fonts.csv: latin, latin-ext only), so it must not be used here. Work Sans, Inter and
Be Vietnam Pro do cover Vietnamese. Work Sans is bundled in `ui/fonts/` (variable woff2, weight 400-700, latin and
vietnamese subsets, licence in `ui/fonts/OFL.txt`) so the app works offline; SF is the fallback. Body 15px / 1.5, rows 15px
medium, descriptions muted, nothing under 12px.

## Layout and controls
8px rhythm, one column (max 640px), cards 24px padding and 16px radius, controls 12px radius and **44px high**.
Check boxes (not switches: switches apply at once, these wait for Apply). Quality as choice cards. Custom
select, never the native one. Visible focus on everything. `prefers-reduced-motion` turns transitions and
animations off.

## Language
Vietnamese (default) and English, a VI | EN switch top right, saved in localStorage. **Every user-facing string
lives in `ui/i18n.js` in both languages**; static HTML uses `data-i18n`, code uses `t(key, {vars})`. Keys are
checked for parity (same keys in both languages, none missing). System errors that are not in the
dictionary are shown as they come. Window title is "PT-E850TKW" (language neutral).

## Pre-delivery checklist (from the skill)
No emoji as icons (inline SVG, one family) · every control has a visible label · errors next to what failed ·
loading and success feedback after Apply · icon buttons have an accessible name · colour is never the only
signal (status has text) · both themes tested.
