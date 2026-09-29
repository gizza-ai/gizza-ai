## About this tool

shadcn/ui components read their colours from CSS custom properties — `--background`,
`--primary`, `--muted-foreground` and friends — declared twice: once in `:root` for light mode and
once in `.dark`. Writing that pair by hand means inventing ~40 values and hoping the text on each
surface stays readable. This generator derives the whole set from one brand colour (plus an
optional accent), and reports the contrast it measured instead of asserting accessibility.

All the colour maths runs in OKLCH, so lightness moves perceptually and the light/dark pair stays
visually matched. Each foreground token is not guessed: the near-white and near-black neutrals are
scored with the WCAG 2.x contrast formula against that surface and the higher-contrast one wins.

**Worked example.** Seed `#6366f1`, `neutral = zinc`, `format = oklch`, `tailwind = v4`,
`radius = 0.625`. Light `--primary` comes back as `oklch(0.585 0.233 277.12)` — the seed itself,
unchanged — with `--primary-foreground` resolving to the near-white neutral, `--radius: 0.625rem`
in `:root`, a `.dark` block whose `--primary` is lifted to `oklch(0.663 0.19 277.12)` so it still
reads on a dark surface, and a trailing comment listing every pair's ratio, e.g.
`foreground on background` at `18.05:1 AA pass`. Paste the whole output into `globals.css`; it is
valid CSS end to end, comments included.

**What it emits.** The upstream token set: `background`, `foreground`, `card`, `popover`,
`primary`, `secondary`, `muted`, `accent` and their `-foreground` partners, plus `destructive`,
`border`, `input`, `ring`, `--radius`, and — when the toggles are on — `--chart-1` … `--chart-5`
and the eight `--sidebar-*` variables. Tailwind v4 also gets the `@theme inline` map that exposes
each token as a `--color-*` utility.

**Limits and edge cases.** `radius` accepts 0 to 2 rem (past ~2rem every shadcn control is already
fully rounded, so the tool rejects it rather than pretending). Colours are quantized to 8-bit sRGB,
so `oklch`, `hsl` and `hex` output describe exactly the same renderable colour — oklch here is a
notation, not a wider gamut. `--destructive` stays red whatever your seed is, because a
brand-tinted delete button is a usability bug. A low-chroma mid-tone seed such as `#808080` cannot
clear 4.5:1 against either near-white or near-black; the tool still generates the theme but marks
the pair `BELOW AA` and adds a warning line. Fonts, live component previews and per-token editing
are out of scope — this is a deterministic generator, not an editor.

## FAQ

<details>
<summary>Does it change my brand colour?</summary>

Not in light mode. `--primary` is your seed verbatim, so the button colour your brand guidelines
specify is the button colour you ship. Only the `.dark` block adjusts it: a dark seed is raised in
OKLCH lightness, because an unmodified dark navy on a dark background is unreadable. The dark value
keeps the original hue and chroma.

</details>

<details>
<summary>Which value notation should I pick?</summary>

Use `oklch` for a current Tailwind v4 project — that is what upstream shadcn ships today. Use `hsl`
if your codebase already stores HSL, and `hex` if you are pasting the colours somewhere that is not
CSS, such as a design file or a README table. All three are quantized from the same sRGB triple, so
switching notation never changes the rendered colour.

</details>

<details>
<summary>What is the difference between the v4 and v3 outputs?</summary>

Tailwind v4 declares tokens as plain values in `:root`/`.dark` and maps them to utilities through an
`@theme inline` block, which this tool appends. Tailwind v3 projects wrap the variable at use site
as `hsl(var(--primary))`, so the variable itself must hold a bare `H S% L%` triplet with no
function around it, inside `@layer base`. Pick `v3` and the output switches to that shape.

</details>

<details>
<summary>How is the contrast report calculated?</summary>

Every foreground/background pair is scored with the WCAG 2.x relative-luminance formula and printed
with its ratio and verdict in the trailing comment. `4.5:1` is the AA threshold for normal-size
text; large or bold text has a lower bar of `3:1`, so a pair marked `BELOW AA` may still be fine for
a heading. Nothing is silently corrected — you see the number and decide.

</details>

<details>
<summary>Why is my accent colour lighter than the hex I typed?</summary>

`--accent` is a surface, not a button: shadcn uses it behind hovered menu items and selected rows,
so a full-strength brand colour there would fight the text on top of it. The accent seed is moved
to a light tint for `:root` and a deep tone for `.dark`, keeping your hue. Leave the accent field
empty to get the neutral accent upstream ships by default.

</details>

<details>
<summary>Can I drop the chart or sidebar variables?</summary>

Yes — untick either toggle. The `--chart-*` group only matters if you use the charts components, and
the `--sidebar-*` group only if you use the sidebar block; leaving them out keeps `globals.css`
shorter. The core token set is always emitted.

</details>
