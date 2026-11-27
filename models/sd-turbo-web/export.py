"""Export SD-Turbo text_encoder / unet / vae_decoder to fp32 ONNX (opset 17, static batch 1, 512x512).

Also writes the shipped shared files into out/: tokenizer/tokenizer.json, LICENSE.md, NOTICE.

IO (all float32 except input_ids int32 / timestep int64), names follow optimum:
  text_encoder: input_ids int32 [1,77] -> last_hidden_state f32 [1,77,1024]
  unet: sample f32 [1,4,64,64], timestep int64 [1], encoder_hidden_states f32 [1,77,1024] -> out_sample f32 [1,4,64,64]
  vae_decoder: latent_sample f32 [1,4,64,64] -> sample f32 [1,3,512,512]

usage: export.py [--src DIR] [--work DIR] [--out DIR] [component ...]
"""
import argparse
import os
import shutil

import onnx
import torch
from diffusers import AutoencoderKL, UNet2DConditionModel
from transformers import CLIPTextModel, CLIPTokenizerFast

from common import COMPONENTS, HERE, add_dirs, resolve_src

OPSET = 17
torch.set_grad_enabled(False)


class TE(torch.nn.Module):
    def __init__(self, m):
        super().__init__()
        self.m = m

    def forward(self, input_ids):
        return self.m(input_ids.to(torch.int64), output_hidden_states=False, return_dict=False)[0]


class UN(torch.nn.Module):
    def __init__(self, m):
        super().__init__()
        self.m = m

    def forward(self, sample, timestep, encoder_hidden_states):
        return self.m(sample, timestep, encoder_hidden_states, return_dict=False)[0]


class VD(torch.nn.Module):
    def __init__(self, m):
        super().__init__()
        self.m = m

    def forward(self, latent_sample):
        return self.m.decode(latent_sample, return_dict=False)[0]


def export(mod, args, names_in, names_out, path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    torch.onnx.export(mod, args, path, input_names=names_in, output_names=names_out,
                      opset_version=OPSET, do_constant_folding=True, dynamo=False)
    m = onnx.load(path)  # loads external data if torch wrote it
    for f in os.listdir(os.path.dirname(path)):
        if f != os.path.basename(path):
            os.remove(os.path.join(os.path.dirname(path), f))
    onnx.save(m, path, save_as_external_data=True, all_tensors_to_one_file=True,
              location=os.path.basename(path) + ".data", size_threshold=1024)
    print("exported", path, flush=True)


def main():
    ap = argparse.ArgumentParser()
    add_dirs(ap)
    ap.add_argument("components", nargs="*", choices=list(COMPONENTS))
    a = ap.parse_args()
    a.components = a.components or list(COMPONENTS)
    src = resolve_src(a.src)

    # fp16 safetensors are the published weights; upcast to fp32 for the export.
    if "text_encoder" in a.components:
        te = CLIPTextModel.from_pretrained(src, subfolder="text_encoder", variant="fp16",
                                           torch_dtype=torch.float32).eval()
        export(TE(te), (torch.zeros(1, 77, dtype=torch.int32),), ["input_ids"], ["last_hidden_state"],
               f"{a.work}/fp32/text_encoder/model.onnx")
        del te
    if "unet" in a.components:
        un = UNet2DConditionModel.from_pretrained(src, subfolder="unet", variant="fp16",
                                                  torch_dtype=torch.float32).eval()
        export(UN(un), (torch.randn(1, 4, 64, 64), torch.tensor([999], dtype=torch.int64),
                        torch.randn(1, 77, 1024)),
               ["sample", "timestep", "encoder_hidden_states"], ["out_sample"],
               f"{a.work}/fp32/unet/model.onnx")
        del un
    if "vae_decoder" in a.components:
        vae = AutoencoderKL.from_pretrained(src, subfolder="vae", variant="fp16",
                                            torch_dtype=torch.float32).eval()
        export(VD(vae), (torch.randn(1, 4, 64, 64),), ["latent_sample"], ["sample"],
               f"{a.work}/fp32/vae_decoder/model.onnx")

    # Shared files for the shipped tree.
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / "tokenizer").mkdir(exist_ok=True)
    CLIPTokenizerFast.from_pretrained(src, subfolder="tokenizer").backend_tokenizer.save(
        str(a.out / "tokenizer" / "tokenizer.json"))
    for f in ("LICENSE.md", "NOTICE"):
        shutil.copyfile(HERE / f, a.out / f)
    print("wrote shared files to", a.out)


if __name__ == "__main__":
    main()
