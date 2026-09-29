## About this tool

An aspect ratio is just width divided by height, but the arithmetic around it is fiddly: resizing a banner without letterboxing, working out what `1366x768` actually reduces to, or checking that a crop is still close enough to 16:9 to pass a platform's spec. This calculator does all three jobs from the same three fields.

Fill in a **ratio plus one dimension** and it solves the missing side. Leave the ratio blank and fill in **both dimensions** and it reduces them to the smallest whole-number ratio. Give a **ratio on its own** and it normalises it and tells you what it is. Give a ratio *and* both dimensions and it shows both ways to reach that ratio — keeping the width, or keeping the height — so you can see which one fits inside the frame you have.

The ratio field is deliberately forgiving. `16:9`, `16/9`, `1920x1080`, `1920×1080`, `1.85:1` and a bare decimal like `1.7778` all mean the same thing, so you can paste a resolution straight from an export dialog instead of reducing it by hand first.

### Worked example

You have a 1920 px wide hero image and the design calls for 16:9. Set `ratio=16:9` and `width=1920`:

```text
Height: 1080 px
Dimensions: 1920 x 1080 px
Aspect ratio: 16:9 (1.7778:1)
Orientation: landscape
Nearest standard: 16:9 — Widescreen HD video (exact match)
Total pixels: 2,073,600 (2.07 MP)
Diagonal: 2,202.91 px
CSS: aspect-ratio: 16 / 9; (legacy padding-top: 56.25%)
```

Going the other way, an old laptop panel at `1366x768` does not reduce to anything memorable. Clear the ratio field, enter both dimensions, and the answer is `683:384` — with the nearest-standard line confirming it is 16:9 to within 0.05%, which is exactly why the panel is sold as widescreen.

### Rounding, and why "even" exists

Ratios rarely divide into whole pixels. 1920 wide at cinema flat 1.85:1 is 1037.8378 px tall, so the tool reports the rounded value and the fraction it came from. The **rounding** control chooses which way that goes: *nearest* for general work, *up* when a crop must never lose content, *down* when a dimension must stay under a budget, and *exact* when you want the unrounded number to feed into something else.

*Even* matters more than it sounds. H.264 and H.265 encode in 2×2 chroma blocks, so ffmpeg and most hardware encoders reject odd widths or heights outright. If the output of this calculator is going into a video pipeline, pick *even* and the dimension is already legal.

### What comes back

Every calculation also names the closest of 25 standard ratios — square, 4:3, 3:2, 16:10, 16:9, 1.85:1, 2.39:1, 21:9, 32:9, 4:5, 9:16, 9:19.5, ISO A-series paper in both orientations, and more — together with how far off you are as a percentage. That is the fastest way to tell whether an odd resolution is "basically 16:9" or genuinely something else.

Switch **output format** when you only want one thing: `dimensions` for a bare `1920x1080` to paste into a resize command, `ratio` or `decimal` for a single value, `css` for an `aspect-ratio` declaration plus the legacy `padding-top` ratio box for older browsers, or `json` for every computed field at once. Dimensions are capped at 1,000,000 px per side. Everything runs as WebAssembly in this tab — nothing you type is uploaded.

## FAQ

<details>
<summary>How do I work out the height for a given width and ratio?</summary>

Put the ratio in the first field, the width in the second, and leave height blank. Height is `width ÷ ratio` — for 1920 at 16:9 that is `1920 ÷ 1.7778 = 1080`. Leaving the width blank instead solves the other direction, `height × ratio`.

</details>

<details>
<summary>What does 1920x1080 reduce to, and how is that found?</summary>

`16:9`. Clear the ratio field, enter both dimensions, and the tool divides both sides by their greatest common divisor — 120 in this case. Ratios written with decimals, like `1.85:1`, are scaled to whole numbers first, which is why `1.85:1` reduces to `37:20`.

</details>

<details>
<summary>Why is my answer 683:384 instead of 16:9?</summary>

Because `1366x768` genuinely is not 16:9 — 1366 is a rounded-up 1365.33. `683:384` is the true reduced ratio, and the "nearest standard" line tells you it sits 0.05% away from 16:9. Panels, thumbnails and social crops are full of these near-misses.

</details>

<details>
<summary>Which rounding mode should I use for video?</summary>

Use `even`. H.264 and H.265 subsample chroma in 2×2 blocks, so encoders reject odd dimensions. For still images `nearest` is fine; use `up` when a crop must not lose any content and `down` when the result has to stay under a size limit.

</details>

<details>
<summary>What is the padding-top percentage in the CSS output for?</summary>

It is the pre-2021 aspect-ratio box trick: a wrapper with `padding-top: 56.25%` holds a 16:9 space open before the image loads. Modern browsers only need `aspect-ratio: 16 / 9`, but the percentage is still handy for email templates and older WebViews. The value is `100 ÷ ratio`.

</details>

<details>
<summary>Can I pass a resolution as the ratio instead of reducing it first?</summary>

Yes — the ratio field accepts `1920x1080` directly. That makes resizing a one-step job: put the source resolution in the ratio field, the new width in the width field, and read off the matching height.

</details>
