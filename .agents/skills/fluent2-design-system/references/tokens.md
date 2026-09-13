# Fluent 2 Design Tokens Reference

All tokens are accessed via `import { tokens } from "@fluentui/react-components"` and used as CSS variable references in `makeStyles`.

## Table of Contents

- [Color Tokens — Neutral](#color-tokens--neutral)
- [Color Tokens — Brand](#color-tokens--brand)
- [Color Tokens — Status/Palette](#color-tokens--statuspalette)
- [Typography Tokens](#typography-tokens)
- [Spacing Tokens](#spacing-tokens)
- [Border Radius Tokens](#border-radius-tokens)
- [Stroke Width Tokens](#stroke-width-tokens)
- [Shadow Tokens](#shadow-tokens)
- [Animation Tokens](#animation-tokens)

---

## Color Tokens — Neutral

### Backgrounds

| Token | Role |
|---|---|
| `colorNeutralBackground1` | Primary surface (cards, page body) |
| `colorNeutralBackground1Hover` | Primary surface hovered |
| `colorNeutralBackground1Pressed` | Primary surface pressed |
| `colorNeutralBackground1Selected` | Primary surface selected |
| `colorNeutralBackground2` | Secondary surface (sidebar, secondary cards) |
| `colorNeutralBackground3` | Tertiary surface |
| `colorNeutralBackground4` | Elevated/floating surfaces |
| `colorNeutralBackground5` | Highest elevation |
| `colorNeutralBackground6` | Canvas/page background |
| `colorNeutralBackgroundDisabled` | Disabled background |
| `colorNeutralBackgroundInverted` | Inverted surface (dark on light) |
| `colorNeutralBackgroundInvertedDisabled` | Inverted surface disabled |
| `colorSubtleBackground` | Transparent at rest |
| `colorSubtleBackgroundHover` | Subtle hover |
| `colorSubtleBackgroundPressed` | Subtle pressed |
| `colorSubtleBackgroundSelected` | Subtle selected |
| `colorTransparentBackground` | Fully transparent |
| `colorTransparentBackgroundHover` | Transparent hover |
| `colorTransparentBackgroundPressed` | Transparent pressed |
| `colorTransparentBackgroundSelected` | Transparent selected |

### Foregrounds

| Token | Role |
|---|---|
| `colorNeutralForeground1` | Primary text (highest contrast) |
| `colorNeutralForeground1Hover` | Primary text hovered |
| `colorNeutralForeground1Pressed` | Primary text pressed |
| `colorNeutralForeground1Selected` | Primary text selected |
| `colorNeutralForeground2` | Secondary text |
| `colorNeutralForeground2Hover` | Secondary text hovered |
| `colorNeutralForeground2Pressed` | Secondary text pressed |
| `colorNeutralForeground2Selected` | Secondary text selected |
| `colorNeutralForeground3` | Tertiary/placeholder text |
| `colorNeutralForeground4` | Quaternary (icons, subtle) |
| `colorNeutralForegroundDisabled` | Disabled text |
| `colorNeutralForegroundOnBrand` | Text on brand background |
| `colorNeutralForegroundInverted` | Text on inverted background |

### Strokes

| Token | Role |
|---|---|
| `colorNeutralStroke1` | Primary border/divider |
| `colorNeutralStroke1Hover` | Border hovered |
| `colorNeutralStroke1Pressed` | Border pressed |
| `colorNeutralStroke1Selected` | Border selected |
| `colorNeutralStroke2` | Secondary border (subtle dividers) |
| `colorNeutralStroke3` | Tertiary border |
| `colorNeutralStrokeDisabled` | Disabled border |
| `colorNeutralStrokeAccessible` | Accessible border (meets 3:1 contrast) |
| `colorNeutralStrokeAccessibleHover` | Accessible border hovered |
| `colorNeutralStrokeAccessiblePressed` | Accessible border pressed |
| `colorNeutralStrokeAccessibleSelected` | Accessible border selected |
| `colorNeutralStrokeOnBrand` | Border on brand surface |
| `colorNeutralStrokeOnBrand2` | Secondary border on brand |
| `colorNeutralStrokeOnBrand2Hover` | Secondary border on brand hovered |
| `colorNeutralStrokeOnBrand2Pressed` | Secondary border on brand pressed |
| `colorNeutralStrokeOnBrand2Selected` | Secondary border on brand selected |

---

## Color Tokens — Brand

| Token | Role |
|---|---|
| `colorBrandBackground` | Primary brand surface (e.g. primary button) |
| `colorBrandBackgroundHover` | Brand surface hovered |
| `colorBrandBackgroundPressed` | Brand surface pressed |
| `colorBrandBackgroundSelected` | Brand surface selected |
| `colorBrandBackground2` | Secondary brand surface (lighter) |
| `colorBrandBackground2Hover` | Secondary brand hovered |
| `colorBrandBackground2Pressed` | Secondary brand pressed |
| `colorBrandBackgroundInverted` | Inverted brand surface |
| `colorBrandBackgroundInvertedHover` | Inverted brand hovered |
| `colorBrandBackgroundInvertedPressed` | Inverted brand pressed |
| `colorBrandBackgroundInvertedSelected` | Inverted brand selected |
| `colorBrandBackgroundStatic` | Non-interactive brand surface |
| `colorBrandForeground1` | Brand text/icon (links, accents) |
| `colorBrandForeground2` | Secondary brand text |
| `colorBrandForegroundLink` | Link text |
| `colorBrandForegroundLinkHover` | Link hovered |
| `colorBrandForegroundLinkPressed` | Link pressed |
| `colorBrandForegroundLinkSelected` | Link selected |
| `colorBrandForegroundInverted` | Brand text on brand surface |
| `colorBrandForegroundInvertedHover` | Brand text on brand hovered |
| `colorBrandForegroundOnLight` | Brand text on light surface |
| `colorBrandForegroundOnLightHover` | Brand text on light hovered |
| `colorBrandStroke1` | Brand border |
| `colorBrandStroke2` | Brand secondary border |
| `colorBrandStroke2Hover` | Brand secondary border hovered |
| `colorBrandStroke2Pressed` | Brand secondary border pressed |
| `colorCompoundBrandBackground` | Compound brand surface (checkboxes, toggles) |
| `colorCompoundBrandBackgroundHover` | Compound brand hovered |
| `colorCompoundBrandBackgroundPressed` | Compound brand pressed |
| `colorCompoundBrandForeground1` | Compound brand foreground |
| `colorCompoundBrandForeground1Hover` | Compound brand foreground hovered |
| `colorCompoundBrandForeground1Pressed` | Compound brand foreground pressed |
| `colorCompoundBrandStroke` | Compound brand stroke |
| `colorCompoundBrandStrokeHover` | Compound brand stroke hovered |
| `colorCompoundBrandStrokePressed` | Compound brand stroke pressed |

---

## Color Tokens — Status/Palette

Each status color (Red, Green, Yellow, Blue, Orange, Marigold, etc.) follows the pattern `colorPalette{Color}{Type}{Variant}`.

### Status Patterns

For each color (`Red`, `Green`, `DarkOrange`, `Yellow`, `Berry`, `Marigold`, `Teal`, etc.):

| Pattern | Example | Role |
|---|---|---|
| `colorPalette{Color}Background1` | `colorPaletteRedBackground1` | Light tinted background |
| `colorPalette{Color}Background2` | `colorPaletteRedBackground2` | Medium tinted background |
| `colorPalette{Color}Background3` | `colorPaletteRedBackground3` | Strong/saturated background |
| `colorPalette{Color}Foreground1` | `colorPaletteRedForeground1` | Primary status foreground |
| `colorPalette{Color}Foreground2` | `colorPaletteRedForeground2` | Secondary status foreground |
| `colorPalette{Color}Foreground3` | `colorPaletteRedForeground3` | Tertiary status foreground |
| `colorPalette{Color}BorderActive` | `colorPaletteRedBorderActive` | Active status border |
| `colorPalette{Color}Border1` | `colorPaletteRedBorder1` | Status border primary |
| `colorPalette{Color}Border2` | `colorPaletteRedBorder2` | Status border secondary |

### Semantic Status Mapping

| Meaning | Color Prefix |
|---|---|
| Danger / Error | `Red` |
| Success | `Green` |
| Warning | `DarkOrange` or `Yellow` |
| Informational | `Blue` (typically via Brand tokens) |
| Severe Warning | `Marigold` |

Also available: `colorStatusDangerBackground1`, `colorStatusDangerForeground1`, `colorStatusSuccessBackground1`, `colorStatusSuccessForeground1`, `colorStatusWarningBackground1`, `colorStatusWarningForeground1`.

---

## Typography Tokens

### Font Family

| Token | Value |
|---|---|
| `fontFamilyBase` | Segoe UI, system fallbacks, sans-serif |
| `fontFamilyMonospace` | Consolas, Courier New, monospace |
| `fontFamilyNumeric` | Bahnschrift, Segoe UI, system fallbacks |

### Font Size (base ramp, in px)

| Token | Size |
|---|---|
| `fontSizeBase100` | 10px |
| `fontSizeBase200` | 12px |
| `fontSizeBase300` | 14px ← Body default |
| `fontSizeBase400` | 16px |
| `fontSizeBase500` | 20px |
| `fontSizeBase600` | 24px |
| `fontSizeHero700` | 28px |
| `fontSizeHero800` | 32px |
| `fontSizeHero900` | 40px |
| `fontSizeHero1000` | 68px |

### Font Weight

| Token | Weight |
|---|---|
| `fontWeightRegular` | 400 |
| `fontWeightMedium` | 500 |
| `fontWeightSemibold` | 600 |
| `fontWeightBold` | 700 |

### Line Height

| Token | Height |
|---|---|
| `lineHeightBase100` | 14px |
| `lineHeightBase200` | 16px |
| `lineHeightBase300` | 20px ← Body default |
| `lineHeightBase400` | 22px |
| `lineHeightBase500` | 28px |
| `lineHeightBase600` | 32px |
| `lineHeightHero700` | 36px |
| `lineHeightHero800` | 40px |
| `lineHeightHero900` | 52px |
| `lineHeightHero1000` | 92px |

### Typography Presets (composite)

Use via `<Text>` preset components or `typographyStyles.*` in `makeStyles`:

| Preset | Size | Weight | Line Height |
|---|---|---|---|
| `caption2` | 10px | Regular | 14px |
| `caption1` | 12px | Regular | 16px |
| `caption1Strong` | 12px | Semibold | 16px |
| `body1` | 14px | Regular | 20px |
| `body1Strong` | 14px | Semibold | 20px |
| `body1Stronger` | 14px | Bold | 20px |
| `body2` | 16px | Regular | 22px |
| `subtitle2` | 16px | Semibold | 22px |
| `subtitle1` | 20px | Semibold | 28px |
| `title3` | 24px | Semibold | 32px |
| `title2` | 28px | Semibold | 36px |
| `title1` | 32px | Semibold | 40px |
| `largeTitle` | 40px | Semibold | 52px |
| `display` | 68px | Semibold | 92px |

Usage in `makeStyles`:

```jsx
import { typographyStyles } from "@fluentui/react-components";

const useStyles = makeStyles({
  heading: typographyStyles.title1,
  body: typographyStyles.body1,
});
```

---

## Spacing Tokens

4px base unit system.

### Horizontal Spacing

| Token | Value |
|---|---|
| `spacingHorizontalNone` | 0 |
| `spacingHorizontalXXS` | 2px |
| `spacingHorizontalXS` | 4px |
| `spacingHorizontalSNudge` | 6px |
| `spacingHorizontalS` | 8px |
| `spacingHorizontalMNudge` | 10px |
| `spacingHorizontalM` | 12px |
| `spacingHorizontalL` | 16px |
| `spacingHorizontalXL` | 20px |
| `spacingHorizontalXXL` | 24px |
| `spacingHorizontalXXXL` | 32px |

### Vertical Spacing

Same scale as horizontal: `spacingVerticalNone` through `spacingVerticalXXXL`, identical values.

---

## Border Radius Tokens

| Token | Value |
|---|---|
| `borderRadiusNone` | 0 |
| `borderRadiusSmall` | 2px |
| `borderRadiusMedium` | 4px |
| `borderRadiusLarge` | 6px |
| `borderRadiusXLarge` | 8px |
| `borderRadiusCircular` | 10000px |

---

## Stroke Width Tokens

| Token | Value |
|---|---|
| `strokeWidthThin` | 1px |
| `strokeWidthThick` | 2px |
| `strokeWidthThicker` | 3px |
| `strokeWidthThickest` | 4px |

---

## Shadow Tokens

Six elevation levels (two layers each):

| Token | Usage |
|---|---|
| `shadow2` | Minimal elevation (tooltips) |
| `shadow4` | Low elevation (cards) |
| `shadow8` | Medium elevation (dropdowns, popovers) |
| `shadow16` | High elevation (dialogs) |
| `shadow28` | Higher elevation (modals) |
| `shadow64` | Highest elevation (full overlays) |

Brand shadow variants: `shadow2Brand`, `shadow4Brand`, `shadow8Brand`, `shadow16Brand`, `shadow28Brand`, `shadow64Brand`.

---

## Animation Tokens

### Duration

| Token | Value |
|---|---|
| `durationUltraFast` | 50ms |
| `durationFaster` | 100ms |
| `durationFast` | 150ms |
| `durationNormal` | 200ms |
| `durationGentle` | 250ms |
| `durationSlow` | 300ms |
| `durationSlower` | 400ms |
| `durationUltraSlow` | 500ms |

### Easing Curves

| Token | Usage |
|---|---|
| `curveAccelerateMax` | Exit animations (fast departure) |
| `curveAccelerateMid` | Moderate exit |
| `curveDecelerateMax` | Enter animations (fast arrival) |
| `curveDecelerateMid` | Moderate enter |
| `curveEasyEase` | General-purpose transitions |
| `curveEasyEaseMax` | Emphasized transitions |
| `curveLinear` | Uniform speed |
