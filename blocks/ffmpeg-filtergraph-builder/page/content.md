## About this tool

**FFmpeg Filtergraph Builder** turns an ordered list of plain filter steps into a
**validated ffmpeg filtergraph string**. You write what you want to happen, in
order; it writes the syntax — commas between filters, colons between options,
quoting for expressions that contain commas, and the `[in]…[out]` pad labels.

**Nothing is executed.** No file is uploaded, no ffmpeg process is started, and no
step you type is ever run — the tool only *composes and checks* the text you'd
paste into your own terminal. Everything happens locally in your browser via
WebAssembly.

### Worked example

Steps:

```
scale to 720p
crop to square
fade in 1s
```

Output (`-filter_complex` form):

```
[0:v]scale=-2:720,crop='min(iw,ih)':'min(iw,ih)',fade=t=in:st=0:d=1[out]
```

Switch **Output form** to *Full ffmpeg command line* and the same steps become:

```
ffmpeg -i input.mp4 -filter_complex "[0:v]scale=-2:720,crop='min(iw,ih)':'min(iw,ih)',fade=t=in:st=0:d=1[out]" -map "[out]" -map "0:a?" output.mp4
```

`-map "0:a?"` keeps the original audio if the file has any, and is skipped
silently if it doesn't.

### Steps you can write

Steps go one per line. You can also separate them with `;` or the word `then`,
and list markers (`-`, `1.`) are ignored, so a pasted recipe usually works as-is.
Filler words (`to`, `the`, `by`, `with`) are ignored too — `scale to 720p` and
`scale 720p` are the same step.

**Video steps**

| Step | Examples | Compiles to |
| --- | --- | --- |
| `scale` | `scale 1280x720`, `scale 720p`, `scale width 1280`, `scale 50%` | `scale=…` |
| `crop` | `crop square`, `crop 640x640`, `crop 16:9`, `crop 80%` | `crop=…` (centred) |
| `pad` | `pad 1920x1080`, `pad 16:9 #101010` | `pad=…` (centred, black by default) |
| `fade` | `fade in`, `fade in 2s`, `fade out 3 at 57` | `fade=t=…:st=…:d=…` |
| `rotate` | `rotate 90`, `rotate 180 degrees`, `rotate 270` | `transpose=…` |
| `flip` | `flip horizontal`, `flip vertical` | `hflip` / `vflip` |
| `grayscale` | `grayscale`, `black and white` | `hue=s=0` |
| `blur` / `sharpen` | `blur`, `blur 12`, `sharpen 1.5` | `gblur=sigma=…` / `unsharp=…` |
| `fps` | `fps 30` | `fps=30` |
| `speed` | `speed 2x`, `speed 0.5` | `setpts=…*PTS` |
| `trim` | `trim 5 to 20` | `trim=…,setpts=PTS-STARTPTS` |
| `reverse` | `reverse` | `reverse` |
| `brightness` / `contrast` / `saturation` / `hue` | `brightness 0.1`, `saturation 1.4`, `hue 90` | `eq=…` / `hue=h=…` |
| `text` | `text "Hello" size 36 color yellow position top box` | `drawtext=…` |
| `raw` | `raw vibrance=intensity=0.5` | passed through, syntax-checked |

**Audio steps** (set *Stream* to **Audio**)

`volume 2` / `volume -6dB`, `fade in 2` / `fade out 3 at 57`, `trim 0 to 30`,
`speed 1.25` (chained `atempo` when the factor is outside 0.5–2), `normalize`
(`loudnorm=I=-16:TP=-1.5:LRA=11`), `mono`, `highpass 120`, `lowpass 8000`,
`reverse`, and `raw`.

### Limits and edge cases

- **One linear chain.** The builder emits a single chain, so multi-input filters
  (`overlay`, `concat`, `amix`) are out of scope — a `;` in a `raw` step is
  rejected for that reason. Use `raw` for a single exotic filter instead.
- **Max 30 steps**, and at most 8,000 characters of step text.
- **Aspect vs. pixels:** a colon means an aspect ratio (`crop 16:9`), an `x`
  means pixels (`crop 640x640`).
- **Caption text** may not contain single quotes, backslashes, or control
  characters — those can't be escaped safely inside a filtergraph — and is
  emitted with `expansion=none` so a `%{…}` sequence stays literal text.
- **File names** in the command form are limited to letters, digits and
  `. _ - + /`, so nothing that could change the meaning of a shell command can
  be pasted in. Rename the file or edit the command by hand for anything else.
- The generated graph is checked for balanced quotes and brackets before it is
  returned, but ffmpeg still has the last word — filter availability depends on
  how your ffmpeg build was compiled (`drawtext` needs `--enable-libfreetype`).

## FAQ

<details>
<summary>Does this run ffmpeg or touch my video?</summary>

No. It's a text tool: steps in, filtergraph string out. No file is read or
uploaded, and no ffmpeg process is ever started — not in the browser, not in the
CLI. You copy the result into your own terminal, where you stay in control of
what runs. If you want a file actually transformed, use one of the dedicated
video or audio tools instead.

</details>

<details>
<summary>What's the difference between the three output forms?</summary>

`-filter_complex` wraps the chain in pad labels — `[0:v]…[out]` — which is what
you need when you map the result explicitly or work with several streams. The
`-vf` / `-af` form is the same chain with no labels, which is the shorter way to
filter one stream. The command form embeds the labelled graph in a complete
`ffmpeg -i … -map …` line you can paste straight into a shell.

</details>

<details>
<summary>Why is `crop to square` written with quotes and `min()`?</summary>

Cropping to a square means "use the shorter side", which is the expression
`min(iw,ih)` — and that expression contains a comma, which ffmpeg would
otherwise read as the separator between two filters. Wrapping it in single
quotes keeps the comma inside the argument, so
`crop='min(iw,ih)':'min(iw,ih)'` is one filter. Crop centres itself when you
don't give `x`/`y`, so no offset is needed.

</details>

<details>
<summary>Can I use a filter the builder doesn't know?</summary>

Yes — write `raw` followed by the filter exactly as ffmpeg expects it, for
example `raw vibrance=intensity=0.5`. It's checked for a valid filter name,
balanced quotes and brackets, and that it's a single filter, then placed in the
chain in order. Filter *names* aren't checked against your ffmpeg build, since
this tool never talks to ffmpeg.

</details>

<details>
<summary>Why does an audio speed change produce several atempo filters?</summary>

Each `atempo` instance is defined for factors between 0.5 and 2. A bigger change
is expressed as a chain whose factors multiply back to what you asked for, so
`speed 4x` becomes `atempo=2,atempo=2` and `speed 3` becomes
`atempo=2,atempo=1.5`. That's the standard way to keep the result in range.

</details>

<details>
<summary>What happens when a step is wrong?</summary>

You get an error naming the step number, the text you wrote, what was expected,
and what was received — for example *step 2 ('sparkle 3'): unknown video step
'sparkle'*, followed by the list of steps that are supported. Nothing partial is
returned, so you never copy a half-built graph.

</details>
