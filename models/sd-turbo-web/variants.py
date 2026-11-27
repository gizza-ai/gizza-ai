"""Build the shipped variants (nof16q8, fp16) from work/fp32.

  fp16    -> ORT transformer fusions + fp16 weights/compute, IO kept float32 (keep_io_types)
  nof16q8 -> fusions + 8-bit MatMulNBits (block 32, asymmetric) on text_encoder and unet
             + int8 per-output-channel weight-only DequantizeLinear on the unet's Conv weights.
             No fp16 anywhere (for devices without shader-f16).

Fusions are conservative: only ops the WebGPU EP has kernels for (LayerNormalization, SkipLayerNormalization
on the text encoder, Gelu family). NhwcConv / GroupNorm / bias fusions are disabled.

usage: variants.py [--work DIR] [--out DIR] [variant ...]
"""
import argparse
import os

import numpy as np
import onnx
from onnx import helper, numpy_helper

from common import COMPONENTS, SHIPPED_VARIANTS, add_dirs

BLOCK = 32
BITS = 8
MODEL_TYPE = {"text_encoder": "clip", "unet": "unet", "vae_decoder": "vae"}
HEADS = {"text_encoder": 16, "unet": 0, "vae_decoder": 0}
HIDDEN = {"text_encoder": 1024, "unet": 0, "vae_decoder": 0}


def save(model, path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    for f in ("model.onnx", "model.onnx.data"):
        p = os.path.join(os.path.dirname(path), f)
        if os.path.exists(p):
            os.remove(p)
    onnx.save(model, path, save_as_external_data=True, all_tensors_to_one_file=True,
              location="model.onnx.data", size_threshold=1024)
    sz = sum(os.path.getsize(os.path.join(os.path.dirname(path), f)) for f in os.listdir(os.path.dirname(path)))
    print(f"saved {path}  total {sz / 2**20:.1f} MiB", flush=True)


def fused(src, comp):
    from onnxruntime.transformers import optimizer
    from onnxruntime.transformers.fusion_options import FusionOptions
    fo = FusionOptions(MODEL_TYPE[comp])
    fo.enable_nhwc_conv = False          # NhwcConv contrib op: not a WebGPU EP kernel
    fo.enable_group_norm = False         # keep the plain InstanceNorm-based subgraph
    fo.enable_skip_group_norm = False
    fo.enable_bias_splitgelu = False
    fo.enable_bias_add = False
    fo.enable_packed_qkv = False
    fo.enable_packed_kv = False
    fo.enable_skip_layer_norm = comp == "text_encoder"
    fo.enable_bias_skip_layer_norm = comp == "text_encoder"
    fo.enable_embed_layer_norm = False
    fo.enable_qordered_matmul = False
    fo.use_multi_head_attention = True
    m = optimizer.optimize_model(f"{src}/{comp}/model.onnx", model_type=MODEL_TYPE[comp],
                                 num_heads=HEADS[comp], hidden_size=HIDDEN[comp],
                                 optimization_options=fo, opt_level=0, use_gpu=False, only_onnxruntime=False)
    print(comp, "fused op counts:", {k: v for k, v in m.get_operator_statistics().items()
                                      if k in ("MultiHeadAttention", "Attention", "LayerNormalization",
                                               "SkipLayerNormalization", "Gelu", "FastGelu", "QuickGelu")})
    return m


def conv_int8_weight_only(model):
    """Conv(W fp32 initializer) -> Conv(DequantizeLinear(W_q int8, scale[oc], zp[oc] int8, axis=0))."""
    g = model.graph
    inits = {t.name: t for t in g.initializer}
    users = {}
    for n in g.node:
        for i in n.input:
            users.setdefault(i, []).append(n)
    new_nodes, n_conv, before, after = [], 0, 0, 0
    for n in g.node:
        if n.op_type == "Conv" and len(n.input) > 1 and n.input[1] in inits:
            wname = n.input[1]
            if len(users[wname]) != 1:
                new_nodes.append(n)
                continue
            w = numpy_helper.to_array(inits[wname]).astype(np.float32)
            oc = w.shape[0]
            amax = np.abs(w.reshape(oc, -1)).max(axis=1)
            scale = np.where(amax > 0, amax / 127.0, 1.0).astype(np.float32)
            q = np.clip(np.round(w / scale.reshape(-1, *([1] * (w.ndim - 1)))), -127, 127).astype(np.int8)
            g.initializer.remove(inits[wname])
            g.initializer.extend([numpy_helper.from_array(q, wname + "_q"),
                                  numpy_helper.from_array(scale, wname + "_scale"),
                                  numpy_helper.from_array(np.zeros(oc, np.int8), wname + "_zp")])
            new_nodes.append(helper.make_node("DequantizeLinear", [wname + "_q", wname + "_scale", wname + "_zp"],
                                              [wname + "_dq"], name=n.name + "_wdq", axis=0))
            n.input[1] = wname + "_dq"
            n_conv += 1
            before += w.nbytes
            after += q.nbytes + scale.nbytes + oc
        new_nodes.append(n)
    del g.node[:]
    g.node.extend(new_nodes)
    print(f"conv int8 weight-only: {n_conv} convs, {before / 2**20:.0f} MiB -> {after / 2**20:.0f} MiB")
    return model


def build_fp16(src, comp):
    m = fused(src, comp)
    m.convert_float_to_float16(keep_io_types=True)
    if not any(o.domain == "com.microsoft" for o in m.model.opset_import):  # the fp16 converter drops it
        m.model.opset_import.append(helper.make_opsetid("com.microsoft", 1))
    return m.model


def build_nof16q8(src, comp):
    from onnxruntime.quantization.matmul_nbits_quantizer import MatMulNBitsQuantizer
    m = fused(src, comp).model
    if comp in ("text_encoder", "unet"):
        qz = MatMulNBitsQuantizer(m, bits=BITS, block_size=BLOCK, is_symmetric=False, accuracy_level=None)
        qz.process()
        m = qz.model.model
        print(comp, "MatMulNBits nodes:", sum(1 for n in m.graph.node if n.op_type == "MatMulNBits"),
              "remaining MatMul:", sum(1 for n in m.graph.node if n.op_type == "MatMul"))
    if comp == "unet":
        m = conv_int8_weight_only(m)
    return m


BUILDERS = {"fp16": build_fp16, "nof16q8": build_nof16q8}


def main():
    ap = argparse.ArgumentParser()
    add_dirs(ap)
    ap.add_argument("variants", nargs="*", choices=list(BUILDERS))
    a = ap.parse_args()
    a.variants = a.variants or list(SHIPPED_VARIANTS)
    src = f"{a.work}/fp32"
    for v in a.variants:
        for comp in COMPONENTS:
            save(BUILDERS[v](src, comp), f"{a.out}/{v}/{comp}/model.onnx")


if __name__ == "__main__":
    main()
