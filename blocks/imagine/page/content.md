## Generate images privately, on your own device

Type a description and press **Generate image**. The picture is drawn by
**Janus-Pro-1B**, an open text-to-image model, running inside your browser
with WebGPU — on your graphics card, not on a server. Your prompt and the
result never leave your device, there's no account, and there's no per-image
cost.

The first time you generate, the browser downloads the model files (about
2.4 GB). They're cached, so later visits start in seconds. Generation then
takes roughly 10–60 seconds per image depending on your GPU; a progress
counter shows how far along it is, and **Cancel** stops it.

### Worked example

Prompt: `A red fox curled up asleep in fresh snow, soft watercolor
illustration`. The model produces one 384 × 384 PNG — a fox in a pale,
loosely painted snowy scene. Each run samples a new image, so pressing
**Generate image** again with the same prompt gives a different variation.

### Writing a good prompt

- **Say what, then how:** the subject first ("a lighthouse on a cliff"), then
  the style ("oil painting", "pixel art", "studio photo").
- **Add light and mood:** "at sunset", "foggy morning", "neon lit".
- **Keep it to one scene.** Short, concrete prompts work better than long
  lists of objects.
- **Try a few runs.** Results vary run to run; generate two or three and keep
  the best.

### Limits and edge cases

- Needs **WebGPU** — a recent desktop Chrome or Edge on a machine with a
  supported GPU. Most phones and some laptops can't run it; the page tells you
  if yours can't.
- Images are **384 × 384** pixels. Upscale or resize afterwards if you need
  them bigger.
- Like all small image models, it struggles with legible text, exact counts,
  hands and complex layouts.
- The first download is large. Use an unmetered connection, and keep the tab
  open until it finishes.

## FAQ

<details>
<summary>Is my prompt sent anywhere?</summary>

No. The model files are downloaded once, but the prompt and the image are processed only in your browser. Nothing is uploaded for inference.
</details>

<details>
<summary>Why does it need WebGPU?</summary>

Generating an image means running a 1-billion-parameter model hundreds of times in a row. WebGPU lets the browser use your graphics card for that; without it the work would take far too long on the CPU.
</details>

<details>
<summary>Why is the first run slow?</summary>

The first run downloads about 2.4 GB of model files. Your browser caches them, so later runs skip straight to generating.
</details>

<details>
<summary>Can I use the images commercially?</summary>

The images come from the open Janus-Pro-1B model. DeepSeek publishes it under its model license, which permits commercial use subject to that license's use restrictions. Read it, and check the rights to anything your prompt depicts, before you publish.
</details>

<details>
<summary>How do I get a bigger image?</summary>

The model draws at 384 × 384. Run the result through an image resizer or upscaler afterwards for larger sizes.
</details>
