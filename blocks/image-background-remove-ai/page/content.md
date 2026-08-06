## Private image background removal

This tool uses the U-2-Netp foreground-segmentation model to isolate the main subject in an image. It works with prominent people, products, animals, vehicles, and other objects, and returns a full-resolution transparent PNG that you can place over a new color, image, presentation, product card, or design.

The model and ONNX runtime download when you first use the tool. Inference then happens inside your browser using WebGPU when available, with a WebAssembly CPU fallback. The selected image is decoded locally and is not uploaded to an AI service.

## How to get a clean cutout

Choose an image with one clear main subject and some contrast between the subject and its surroundings. Clean edges, good lighting, and an uncluttered background usually produce the best mask. Very low contrast, motion blur, heavy occlusion, transparent objects, or several equally prominent subjects can make the edge less accurate.

This is a general foreground remover, so it decides which visually dominant subject to keep. It does not let you click a particular object in a complex scene, and it is not intended to erase one object while preserving the rest of the background.

If the model finds almost no foreground, the page reports that no foreground subject was detected instead of offering an empty PNG. Try an image with a clearer subject or stronger foreground/background contrast.

## FAQ

<details>
<summary>Is my photo uploaded?</summary>

No. The model files download to the browser, but your image stays on your device and inference runs locally in the page.

</details>

<details>
<summary>Why is the first run slower?</summary>

The browser must download and initialize the pinned U-2-Netp model and shared ONNX runtime. The model itself is about 5 MB. Browser caching makes later runs much faster unless site data is cleared.

</details>

<details>
<summary>Does it work without WebGPU?</summary>

Yes. The page tries WebGPU first and falls back to WebAssembly on the CPU. The fallback is slower, particularly for large photos, but it does not require a compatible GPU.

</details>

<details>
<summary>What output format does it create?</summary>

The result is a PNG with an alpha channel. Transparent areas may appear as a checkerboard in the preview and remain transparent when downloaded.

</details>

<details>
<summary>Can it remove a product or animal background?</summary>

Yes. The model is designed to find a prominent foreground subject rather than only a person. Products and animals with distinct outlines and contrasting backgrounds work best; fine fur, reflections, transparent materials, and busy scenes can still need a more specialized editor.

</details>
