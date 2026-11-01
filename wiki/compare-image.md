# Comparing a screenshot with the design

`figlens compare-image <file.fig> <node> <screenshot.png> [--scale N] [--limit N] [--out diff.png] [--full-page] [--ignore ids] [--threshold N]` (MCP `compare_image`: `file`, `node`, `image`, `scale`, `limit`, `out`, `fullPage`, `ignore`, `threshold`) measures a screenshot of the built screen against the node as figlens renders it, so visual checks can be "Proven by a tool". `compare` diffs a REST `nodes.json`, not pixels. CLI and MCP share `command::compare_image`.

## How it compares

- The node is rendered over its layout box (else the export area), cut to it: a screenshot shows the box, not shadows past it. Scale = screenshot width / node width, or `--scale`; one narrower than the node, or more than 1 px off `round(width × scale)`, is refused.
- A taller screenshot is compared over the node's height, the rest reported as `uncompared.below`. One shorter by over 1% (and 2 px) is refused with a full-page hint, so a viewport capture cannot score only the visible part.
- `--full-page` (MCP `fullPage`) states the screenshot is the whole page, so a shorter page is a finding, not a capture mistake: the 1%/2 px slack drops to 1 px of rounding, its rows are compared, and the frame rows it lacks are reported as `uncompared.missing` (device px), counted as differing in `mismatch` and `nonTextMismatch` (whose denominators then include them), and form one region of their own (never `text`, never dropped as noise, never merged with differences above) naming the layers there. Padding the viewport to the frame's height instead hides the missing height in ordinary background mismatch.
- Both images are composited over white. A pixel differs when no pixel within one CSS pixel (`ceil(scale)` device pixels, at most 4) in the other image is within the threshold (48/255 by default, `--threshold` 1-254) on every channel, checked both ways: anti-aliasing and sub-pixel edges drop out.
- Differing pixels are counted per 8 × 8 node-pixel cell; 8-connected cells form regions, and regions under 12 node px² are dropped. `mismatch` counts every differing pixel; `nonTextMismatch`/`textMismatch` split the kept regions' pixels by cell.
- Each cell is blamed on the smallest of the deepest visible layers (boxes cut by clipping ancestors) overlapping its pixels; a region names up to three layers with the most. A cell over a TEXT layer is a text cell; a region with at least 90% text-cell pixels is `text: true`: geometry only, so a wrong text color (an inverted label) lands there with font fallback; each needs a cause. A cell whose deepest layers all have a visible image fill is an image cell; a region with at least 90% is `image: true` (banners, avatars, logos), split out as `imageMismatch`/`nonImageMismatch`.
- `--ignore` (MCP `ignore`) takes layers strictly inside the node; each one's render box is left out of every count, the denominator and the regions, its subtree is never blamed, and `ignored` lists the layers and pixels. A layer with no render box (hidden, a mask, clipped away) is refused: its layout box could cover content that draws. The ignored layer's own size and position are no longer measured; check them with `inspect`. In the diff PNG ignored pixels are flat gray.
- `warnings` are the node's known skipped or approximated rendering, as in `export`. `--out` writes the washed-out screenshot, differing pixels red, regions outlined red (layout), blue (text) or purple (image), via `output::write`; the design and the screenshot are refused as outputs.

The screenshot is untrusted: at most 64 MiB is read, and the PNG signature and IHDR size (at most 25,000,000 pixels) are checked before decoding, then the decoded size. The render at that scale has the same cap. Decoding uses resvg's `tiny_skia`, no new dependency.

## Measured (2026-11-01)

| Input | mismatch | nonText | regions (non-text) |
|---|---:|---:|---:|
| Fixture `0:1` render vs itself at 1x/2x; a private 1440 × 1024 frame vs its render | 0 | 0 | 0 |
| Browser build of a private chat screen, fallback fonts, light / dark | 1.61% / 1.54% | 0.33% / 0.35% | 30 (5) / 27 (6) |
| Browser build of a private dashboard, fonts loaded, corrected | 0.45% | 0.14% | 49 (21) |
| The same build's first pass (content offset vertically) | 7.06% | 3.03% | 102 (77) |

The chat build's largest non-text region is a real 1 px border drawn inside the frame, not outside. A button moved 14 px in a render, moved 20 px in the browser shot, or recolored, each gave a non-text region on its layers.

Thresholds 16/24/32 raised the chat build's `nonTextMismatch` to 1.49/1.30/0.80% and the dashboard's to 1.34/0.38/0.18%; 48 keeps faithful builds low and still separates the first pass. Cost: by default, differences under 48/255, such as that button's light gray border on off-white, are not seen; `--threshold 16` shows them, and `inspect --colors` checks the values.

## When to mask or lower the threshold

- A frame mixes layout with content from data (back-office banners, user names, balances, empty lists on a test account): those differ by data, so the score cannot show a layout fix. Pass their layer ids to `--ignore`, then fix what remains; report what was ignored and why.
- `image: true` regions without `--ignore` are content to confirm by eye (right asset, crop, fit), not layout.
- 48 hides light strokes, dividers and shadows (`#e4edff` on white is 27/255). After the default run is clean, rerun with `--threshold 16` (lower for fainter shadows) and read only new regions over those layers; expect more anti-aliasing noise, so do not compare its score with a default run.

## Limits

Not perceptual; one viewport and state per screenshot; crop browser chrome or a wider page to the frame. A long thin difference gets a large box: read `pixels`. Agent use: skill Verification evidence.
