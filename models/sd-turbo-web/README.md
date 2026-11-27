# sd-turbo-web

Conversion recipe: `stabilityai/sd-turbo` -> static 512x512 ONNX for onnxruntime-web (WebGPU), in two
shipped variants. Only the recipe is committed; model weights and `out/` / `work/` never are.

Pinned upstream: `stabilityai/sd-turbo` @ `b261bac6fd2cf515557d5d0707481eafa0485ec2` (constant `REVISION`
in `common.py`). The model is under the Stability AI Community License: `LICENSE.md` is copied verbatim
from that revision and `NOTICE` carries the "Powered by Stability AI" attribution. Both are copied into
`out/` and listed in the manifest.

## Run

```sh
cd models/sd-turbo-web
uv run python export.py      # download pinned weights, export fp32 ONNX to work/, write shared files to out/
uv run python variants.py    # build out/nof16q8 and out/fp16 from work/fp32
uv run python validate.py    # PSNR vs the diffusers fp32 pipeline; writes out/manifest.json on pass
```

All three take `--work DIR` and `--out DIR`; `export.py` and `validate.py` take `--src DIR` to use an
existing local snapshot of the pinned revision (fp16 safetensors layout) instead of downloading. CPU only;
needs roughly 10 GB of disk (5 GB for `work/`) and a few GB of RAM.

## Output tree

```
out/
  tokenizer/tokenizer.json            CLIP tokenizer (fast, single file)
  LICENSE.md  NOTICE
  manifest.json
  nof16q8/{text_encoder,unet,vae_decoder}/model.onnx + model.onnx.data
  fp16/{text_encoder,unet,vae_decoder}/model.onnx + model.onnx.data
```

Model IO (all float32 except `input_ids` int32 and `timestep` int64):

| component | inputs | output |
| --- | --- | --- |
| text_encoder | `input_ids` [1,77] | `last_hidden_state` [1,77,1024] |
| unet | `sample` [1,4,64,64], `timestep` [1], `encoder_hidden_states` [1,77,1024] | `out_sample` [1,4,64,64] |
| vae_decoder | `latent_sample` [1,4,64,64] | `sample` [1,3,512,512] |

Export: opset 17, static shapes, external data, fp16 source weights upcast to fp32.

## Variants

- `fp16`: ORT transformer fusions (LayerNormalization, SkipLayerNormalization on the text encoder, Gelu
  family; no NhwcConv/GroupNorm/bias fusions), weights and compute in fp16, float32 IO. The `com.microsoft`
  opset import that the fp16 converter drops is restored. Needs `shader-f16`.
- `nof16q8`: same fusions, 8-bit MatMulNBits (block 32, asymmetric) on text encoder and unet, int8
  per-channel weight-only DequantizeLinear on unet convs, no fp16 anywhere (for devices without
  `shader-f16`).

## Validation

`validate.py` runs a fixed case (prompt "a photo of a red fox sitting in a snowy forest, highly detailed",
seed-42 noise, 1 Euler step, guidance 0) through each variant with onnxruntime CPU and compares with the
diffusers fp32 image. It exits non-zero unless `nof16q8` >= 33 dB and `fp16` >= 55 dB PSNR.
`manifest.json` is written only when both pass.

## manifest.json schema

```json
{
  "schema": 1,
  "model": "stabilityai/sd-turbo",
  "revision": "<pinned sha>",
  "shared": [ { "path": "tokenizer/tokenizer.json", "bytes": 0, "sha256": "<hex>" } ],
  "variants": {
    "nof16q8": { "files": [ { "path": "nof16q8/unet/model.onnx", "bytes": 0, "sha256": "<hex>" } ] },
    "fp16":    { "files": [ ... ] }
  }
}
```

`path` is relative to `out/` (POSIX separators) for every file; `bytes` is the exact size and `sha256` the
lowercase hex digest. `shared` lists `tokenizer/tokenizer.json`, `LICENSE.md` and `NOTICE`; each variant lists
all six of its `model.onnx` / `model.onnx.data` files. Sum a variant's `bytes` (plus `shared`) for download
progress totals.
