## About this tool

Convert between the two standard glTF 2.0 containers without uploading a model anywhere. Paste a text `.gltf` JSON document to pack it into a binary `.glb`, or paste GLB bytes as base64, hex, or a `data:model/gltf-binary;base64,...` URL to unpack it back to glTF JSON.

The converter copies accessor bytes unchanged. It can merge multiple buffers into one GLB `BIN` chunk, pack a supplied external `.bin` buffer into the output, extract that buffer back out, and move simple texture image bytes between buffer views and `data:` URIs. Choose `output=summary` when you want a quick report instead of the converted file text.

### Worked example

For a tiny valid glTF scene with no geometry, paste:

```
{"asset":{"version":"2.0"},"scenes":[{"nodes":[]}] }
```

Use `input_format=gltf`, `to=glb`, and `output=summary`. The report shows a `glTF JSON -> GLB` direction, the JSON chunk size, scene count, and warnings if the model has features the converter intentionally leaves alone. Switch `output=file` to receive the GLB as a data URL, or set `output_encoding=base64` / `hex` when you want raw encoded bytes for a pipeline.

### Limits and edge cases

- Pasted text is capped at **24 MiB** and any decoded binary input is capped at **16 MiB**.
- This is a container converter, not a mesh optimizer. It does not simplify geometry, quantize accessors, weld vertices, or recompress textures.
- Draco and meshopt-compressed buffer views are not decompressed. When relocation would break them, the tool returns a clear error.
- External buffers and textures must be pasted into fields as base64, hex, or data URLs. The browser page cannot read sibling files from your disk automatically.
- Only glTF 2.0 is supported. Older glTF 1.x assets are rejected rather than upgraded silently.

## FAQ

<details>
<summary>What is the difference between glTF and GLB?</summary>

A `.gltf` file is JSON and often points at separate `.bin` buffers and texture images. A `.glb` file stores the JSON chunk and the binary buffer chunk in one binary file, which is convenient for uploads, sharing, and web delivery.

</details>

<details>
<summary>How do I convert a GLB if the page only accepts text?</summary>

Encode the GLB bytes as base64 or hex first, then paste that text into `model`. A `data:model/gltf-binary;base64,...` URL also works. Use `input_format=auto` for normal data, or force `base64` / `hex` if you know the encoding.

</details>

<details>
<summary>Can this pack a glTF file that references scene.bin?</summary>

Yes, if you paste the contents of `scene.bin` into the `bin` field as base64, hex, or a data URL. The converter has no file picker for sibling assets, so it cannot read `scene.bin` from disk by name.

</details>

<details>
<summary>Does it preserve the exact geometry bytes?</summary>

Yes for ordinary uncompressed buffers: accessor and bufferView bytes are copied byte for byte, with only padding and container metadata adjusted as needed. It will not decode or rewrite Draco or meshopt-compressed geometry.

</details>

<details>
<summary>When should I use output=buffer?</summary>

Use `output=buffer` after unpacking to glTF with a `buffer_uri` such as `scene.bin`. The first run gives you the JSON that references `scene.bin`; the buffer output gives you the binary bytes to save beside it.

</details>
