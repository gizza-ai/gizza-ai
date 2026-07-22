# gamma-correct — competitor analysis (2026-07-22)

Tool function: apply a gamma/power-law curve to image midtones without clipping endpoints. The
implementation uses ffmpeg's `eq` filter, which supports overall gamma, per-channel gamma, and
`gamma_weight` highlight protection.

## Competitors skimmed (paraphrased)

1. Browser image editors with curves/gamma controls: expose a single gamma or midtone slider,
   preview/download, and format conversion.
2. ImageMagick/command-line examples: `-gamma` for overall correction and per-channel variants,
   commonly documented with values such as 0.5, 1.0, 1.8, 2.2.
3. ffmpeg `eq` filter references and GUI wrappers: gamma, gamma_r/g/b, gamma_weight, brightness,
   contrast, saturation. Brightness/contrast/saturation are broader color-adjustment controls, not
   necessary for this focused tool.

## Table-stakes → decision

| Capability | Competitors | Our decision |
|---|---|---|
| Overall gamma | all | **in-model** — `gamma` number 0.1-10, slider, defaults to 1 |
| Clear brighten/darken semantics | all | **in-model** — docs and presets explain >1 brightens, <1 darkens |
| Per-channel correction | ImageMagick/ffmpeg | **in-model** — `gamma_r`, `gamma_g`, `gamma_b` numbers 0.1-10 |
| Highlight protection | ffmpeg eq | **in-model differentiator** — `gamma_weight` 0-1 |
| Output format choice | browser tools | **in-model** — keep/png/jpg/webp enum |
| Presets/examples | browser tools | **in-model** — chips for brighten, darken, warm cast, highlight protection |
| Curves histogram UI | full editors | **out-of-model** — declarative page has no histogram/curve canvas; listed, not built |
| Batch image processing | some editors | **out-of-model** — single-file tool model |
| Brightness/contrast/saturation suite | ffmpeg wrappers | **out-of-scope** — separate broader image adjustment tools can own those controls |

## Defaults and verification notes

- Defaults (`gamma=1`, channel gammas `1`, `gamma_weight=1`, `format=keep`) are identity.
- Gamma boundary values 0.1 and 10 are accepted; outside values are rejected before ffmpeg.
- Page tests use a mid-gray fixture so a real gamma curve is measurable: gamma 1.8 brightens the
  center sample; gamma 0.5 darkens it. Format tests assert MIME/download extension.
