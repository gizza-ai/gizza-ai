## About this tool

HDR video is mastered for wide-gamut BT.2020 color and a high peak brightness using
a PQ (`smpte2084`) or HLG (`arib-std-b67`) transfer curve. Play that same file on an
ordinary **standard-dynamic-range** screen — or drop it into an editor or a website
that doesn't understand HDR — and it renders **gray, dim, over-dark, or washed-out**,
because the SDR display reads the HDR signal as if it were plain BT.709. This tool
**tone-maps** the clip down to SDR BT.709 / `yuv420p` so it looks right everywhere.

It runs a fixed, order-sensitive ffmpeg `zscale` + `tonemap` filter chain: linearize
the source transfer → convert to float RGB → gamut-map to BT.709 → tone-map the
luminance with your chosen curve → re-encode the BT.709 transfer at limited (`tv`)
range → 8-bit 4:2:0. Everything runs in your browser via WebAssembly ffmpeg — the
file is never uploaded.

### Controls

- **Tone-map curve** — *Hable* is the filmic default every HDR→SDR guide recommends;
  it preserves highlight and shadow detail with natural contrast. *Mobius* keeps more
  midtone saturation. *Reinhard* is a flatter, brighter global operator. *Linear* and
  *Clip* are the simplest (and can dim or blow out highlights).
- **Peak luminance (nits)** — the target display's nominal peak white. **100** is the
  reference SDR white point and the right default; raise it only if you're mastering
  for a brighter SDR target.
- **Highlight desaturation** — `0` keeps highlight color (recommended). A small value
  (e.g. `0.5`) grays very bright areas to tame neon/clipped highlights.
- **Output format** — **MP4** (H.264 + AAC) for maximum compatibility, or **WebM**
  (VP9 + Opus). **Quality** is 1-100 and maps to the encoder's CRF (75 ≈ high-quality
  delivery; lower = smaller file).

### Worked examples

- **A phone HDR clip that looks washed-out on your laptop:** keep every default —
  *Hable*, peak *100*, desat *0*, *MP4*, quality *75*. The filter chain becomes
  `zscale=t=linear:npl=100,...,tonemap=tonemap=hable:desat=0,...,format=yuv420p` and
  you get a natural-looking SDR MP4.
- **Punchier grade for social:** pick *Mobius* and quality *80* to hold more midtone
  saturation.
- **For the web with alpha-free VP9:** switch **Output format** to *WebM* — the exact
  same tone-map runs, only the codec changes to VP9/Opus.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions. -->

<details>
<summary>Why does my HDR video look gray or dim on a normal screen?</summary>

Because the file carries an **HDR transfer curve** (PQ or HLG) and **BT.2020** wide-gamut
color, but a standard-dynamic-range display decodes it as ordinary BT.709. The bright,
wide-gamut signal gets squashed into the SDR range with no tone-mapping, so highlights
flatten and midtones look washed-out or dark. Tone-mapping remaps the luminance and
gamut into BT.709 so the picture looks correct.

</details>

<details>
<summary>Which tone-map curve should I pick?</summary>

Start with **Hable** — it's a filmic S-curve that keeps highlight and shadow detail with
natural contrast, and it's the default in almost every HDR→SDR guide. Try **Mobius** if
you want punchier, more saturated midtones, or **Reinhard** for a flatter, brighter look.
**Linear** and **Clip** are the simplest operators; *Clip* hard-clips bright highlights
(sharpest but can blow them out), *Linear* just scales luminance.

</details>

<details>
<summary>What does "peak luminance (nits)" do?</summary>

It's the nominal peak brightness of the SDR target you're mapping *to* — the reference
white the tone-map curve rolls the HDR highlights down toward. **100 nits** is the
standard SDR white point and the right value for almost everyone. Raising it targets a
brighter SDR display and leaves highlights a little brighter (and closer to clipping).

</details>

<details>
<summary>What can I put in, and what comes out?</summary>

Input can be any HDR (or SDR) video your browser can decode — MP4/MOV/MKV with PQ or HLG
HDR, BT.2020. Output is **MP4 (H.264 + AAC)** by default, or **WebM (VP9 + Opus)** — both
SDR BT.709 / 8-bit 4:2:0, playable on any ordinary screen. Audio is re-encoded, not
dropped.

</details>

## Limits

- Runs entirely in your browser via WebAssembly ffmpeg; large inputs are bounded by your
  browser's memory, and very long or high-resolution clips encode slowly on the CPU.
- Tone-mapping is one-way — it maps HDR *down* to SDR and can't recover HDR from an SDR
  source. The tone-map curve and peak are heuristics, not a per-shot color grade.
- Output is always 8-bit SDR BT.709 (`yuv420p`); it does not preserve HDR metadata or
  10-bit color. Only MP4 and WebM containers are written.
