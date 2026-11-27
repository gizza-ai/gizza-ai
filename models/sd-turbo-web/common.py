"""Constants and helpers shared by export.py, variants.py and validate.py."""
import argparse
import hashlib
import os
from pathlib import Path

HERE = Path(__file__).resolve().parent

MODEL_ID = "stabilityai/sd-turbo"
# Pinned upstream revision. Every download and the shipped LICENSE.md are from this commit.
REVISION = "b261bac6fd2cf515557d5d0707481eafa0485ec2"

COMPONENTS = ("text_encoder", "unet", "vae_decoder")
SHIPPED_VARIANTS = ("nof16q8", "fp16")

# Fixed validation case.
PROMPT = "a photo of a red fox sitting in a snowy forest, highly detailed"
SEED = 42


def add_dirs(ap: argparse.ArgumentParser) -> None:
    ap.add_argument("--src", type=Path, default=None,
                    help="local directory holding a stabilityai/sd-turbo snapshot at the pinned "
                         "revision (skips the Hugging Face download)")
    ap.add_argument("--work", type=Path, default=HERE / "work", help="intermediate fp32 ONNX (not shipped)")
    ap.add_argument("--out", type=Path, default=HERE / "out", help="shipped tree")


def resolve_src(src: Path | None) -> Path:
    """Return a local snapshot dir of the sd-turbo pipeline (fp16 safetensors only), downloading if needed."""
    if src is not None:
        return src
    from huggingface_hub import snapshot_download
    return Path(snapshot_download(
        MODEL_ID, revision=REVISION,
        allow_patterns=["model_index.json", "scheduler/*", "tokenizer/*", "LICENSE.md", "*/config.json",
                        "text_encoder/model.fp16.safetensors",
                        "unet/diffusion_pytorch_model.fp16.safetensors",
                        "vae/diffusion_pytorch_model.fp16.safetensors"]))


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def files_under(root: Path) -> list[Path]:
    return sorted(p for p in root.rglob("*") if p.is_file())
