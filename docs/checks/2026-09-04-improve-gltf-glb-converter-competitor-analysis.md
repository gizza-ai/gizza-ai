# gltf-glb-converter — competitor analysis (2026-09-04)

Scan run before finishing the implementation. Web search: "online glTF to GLB converter glb to gltf converter options embed buffers textures". The top reachable tool pages were skimmed and the notes below are paraphrased.

## Competitors reviewed

### 1. Any3D glTF to GLB converter
- Inputs: a `.gltf` asset, with the page describing the usual need to bundle external buffers and textures.
- Outputs: a downloadable `.glb` intended for easier sharing and web delivery.
- UX: upload/select a file, convert, then download; the copy emphasizes local/browser processing and privacy.
- Limitations visible from the public page: the primary flow is glTF → GLB, not a symmetric GLB → glTF unpacker.

### 2. 3DConvert glTF to GLB converter
- Inputs: a text glTF file plus its associated external assets.
- Outputs: a packed binary GLB.
- UX: simple upload-and-convert workflow with privacy/local-processing positioning.
- Table stakes: binary bundling of buffers is the core value; no fine-grained JSON/chunk inspection options were visible.

### 3. Sculpty glTF to GLB converter
- Inputs: glTF scene assets.
- Outputs: GLB that bundles JSON, buffers and textures into one file.
- UX: file-first workflow aimed at sharing, hosting, and versioning 3D models.
- Table stakes: one-file output and texture/buffer bundling are highlighted; advanced optimization is treated as a separate product category.

## Table-stakes checklist and decisions

| Capability | Seen at | In/out of model | Decision |
| --- | --- | --- | --- |
| Convert glTF JSON to GLB | all three | in-model | `to=glb` or `to=auto` packs JSON and buffers into GLB |
| Accept external `.bin` buffer | all three imply it | in-model | `bin` field accepts base64/hex/data URI bytes for one external buffer |
| Bundle buffers into one file | all three | in-model | multi-buffer inputs are merged and bufferViews remapped |
| GLB to glTF unpacking | less visible on top pages | in-model | included as `to=gltf`, because the same container logic can unpack byte-exactly |
| Binary input encodings | needed for chat/page model | in-model | GLB bytes accepted as base64, hex, or data URL |
| Texture relocation | Any3D/Sculpty emphasize textures | in-model with caveats | `images=buffer` packs data URI images, `images=uri` extracts bufferView images to data URIs |
| Downloadable binary result | all three | in-model | binary output is data URL by default, with base64/hex for pipelines |
| Human-readable report | converter pages typically show status | in-model | `output=summary` reports direction, sizes, counts, and warnings |
| Pretty glTF JSON | developer convenience | in-model | `pretty` checkbox defaults true for JSON output |
| Draco/meshopt decompression | common 3D ecosystem need | out-of-model | listed as unsupported; this tool does container conversion, not geometry decompression |
| Mesh optimization/quantization | dedicated optimizer tools | out-of-model | not built; no geometry re-encoding or simplification |
| Direct multi-file upload | competitor browser UX | out-of-model here | page has text fields only; external bytes must be pasted as base64/hex/data URI |
| 3D preview | some 3D sites offer previews | out-of-model | no renderer/viewer shipped in this toolkit block |

## Design conclusions carried into the descriptor

- The tool must be explicit that it is a container converter, not a geometry optimizer.
- Because this toolkit's chat/page surface is text-only for pure tools, binary GLB input and external assets are encoded as base64, hex, or data URLs.
- `to=auto` handles the common "flip the container" case, while fixed `to=glb` and `to=gltf` support repacking.
- `output=summary` gives a short auditable report for tests and quick checks; `output=file` returns the converted model.
- Unsupported compression extensions are called out rather than silently corrupting bufferView references.
