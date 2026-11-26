"""Validate the shipped variants against the diffusers fp32 pipeline and write out/manifest.json.

Fixed case: PROMPT, seed-42 noise, one Euler step, guidance 0 (SD-Turbo). Each variant is run with
onnxruntime CPU through the same manual loop the browser worker implements (text encoder, unet, x0 =
lat - sigma*eps, vae decode) and scored by PSNR (dB, 0..1 pixel scale) against the diffusers image.

Exits non-zero unless nof16q8 >= 33 dB and fp16 >= 55 dB. manifest.json is written only on a pass.

usage: validate.py [--src DIR] [--out DIR]
"""
import argparse
import json
import sys

import numpy as np
import onnxruntime as ort
import torch

from common import COMPONENTS, MODEL_ID, PROMPT, REVISION, SEED, SHIPPED_VARIANTS, add_dirs, files_under, \
    resolve_src, sha256_file

THRESHOLDS_DB = {"nof16q8": 33.0, "fp16": 55.0}


def reference(src):
    from diffusers import EulerDiscreteScheduler, StableDiffusionPipeline
    torch.set_grad_enabled(False)
    pipe = StableDiffusionPipeline.from_pretrained(src, variant="fp16", torch_dtype=torch.float32,
                                                   safety_checker=None, requires_safety_checker=False)
    assert isinstance(pipe.scheduler, EulerDiscreteScheduler)
    noise = torch.randn((1, 4, 64, 64), generator=torch.Generator().manual_seed(SEED), dtype=torch.float32)
    img = pipe(PROMPT, num_inference_steps=1, guidance_scale=0.0, latents=noise.clone(),
               height=512, width=512, output_type="np").images[0]
    sch = pipe.scheduler
    sch.set_timesteps(1)
    ids = pipe.tokenizer(PROMPT, padding="max_length", max_length=77, truncation=True,
                         return_tensors="np").input_ids.astype(np.int32)
    return {"img": img.astype(np.float32), "noise": noise.numpy(), "ids": ids,
            "timestep": int(sch.timesteps[0]), "sigma": np.float32(sch.sigmas[0]),
            "init_noise_sigma": np.float32(sch.init_noise_sigma),
            "scaling_factor": float(pipe.vae.config.scaling_factor)}


def psnr(a, b):
    mse = float(np.mean((a.astype(np.float64) - b) ** 2))
    return 10 * np.log10(1 / mse) if mse > 0 else float("inf")


def session(path):
    so = ort.SessionOptions()
    so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    return ort.InferenceSession(path, so, providers=["CPUExecutionProvider"])


def run_variant(root, r):
    sigma = r["sigma"]
    emb = session(f"{root}/text_encoder/model.onnx").run(None, {"input_ids": r["ids"]})[0]
    lat = r["noise"] * r["init_noise_sigma"]
    eps = session(f"{root}/unet/model.onnx").run(None, {
        "sample": (lat / np.sqrt(sigma ** 2 + 1)).astype(np.float32),
        "timestep": np.array([r["timestep"]], np.int64), "encoder_hidden_states": emb})[0]
    x0 = lat - sigma * eps
    img = session(f"{root}/vae_decoder/model.onnx").run(
        None, {"latent_sample": (x0 / r["scaling_factor"]).astype(np.float32)})[0]
    return np.clip(img / 2 + 0.5, 0, 1)[0].transpose(1, 2, 0)


def write_manifest(out):
    def entry(p):
        return {"path": p.relative_to(out).as_posix(), "bytes": p.stat().st_size, "sha256": sha256_file(p)}
    shared = [out / "tokenizer" / "tokenizer.json", out / "LICENSE.md", out / "NOTICE"]
    manifest = {
        "schema": 1,
        "model": MODEL_ID,
        "revision": REVISION,
        "shared": [entry(p) for p in shared],
        "variants": {v: {"files": [entry(p) for p in files_under(out / v)]} for v in SHIPPED_VARIANTS},
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print("wrote", out / "manifest.json")


def main():
    ap = argparse.ArgumentParser()
    add_dirs(ap)
    a = ap.parse_args()
    r = reference(resolve_src(a.src))
    ok = True
    for v in SHIPPED_VARIANTS:
        for comp in COMPONENTS:
            for f in ("model.onnx", "model.onnx.data"):
                if not (a.out / v / comp / f).exists():
                    sys.exit(f"missing {a.out / v / comp / f}: run export.py and variants.py first")
        db = psnr(run_variant(a.out / v, r), r["img"])
        passed = db >= THRESHOLDS_DB[v]
        ok &= passed
        print(f"{'PASS' if passed else 'FAIL'} {v}: {db:.2f} dB (need >= {THRESHOLDS_DB[v]:.0f} dB)", flush=True)
    if not ok:
        sys.exit(1)
    write_manifest(a.out)


if __name__ == "__main__":
    main()
