import { test, expect } from './fixtures';

// /tools/imagine/ runs Janus-Pro on WebGPU via the shared model worker.
// Headless Chromium has no WebGPU adapter, so the real model never loads here;
// these specs cover the page contract around it.

test('imagine page explains its download and WebGPU need up front', async ({ page }) => {
  await page.goto('/tools/imagine/');
  await expect(page.locator('.tool-model-note')).toContainText('about 2.4 GB of model files');
  await expect(page.locator('.tool-model-note')).toContainText('Needs WebGPU');
  await expect(page.locator('#tool-generate')).toHaveText('Generate image');
});

test('imagine page refuses clearly without a WebGPU adapter', async ({ page }) => {
  await page.goto('/tools/imagine/');
  await page.fill('#in-prompt', 'a red fox in the snow');
  await page.click('#tool-generate');
  await expect(page.locator('#tool-output')).toContainText('Image generation needs WebGPU', { timeout: 60_000 });
  await expect(page.locator('#tool-generate')).toHaveText('Generate image');
  await expect(page.locator('#tool-output-media')).toBeHidden();
});

test('imagine page asks for a prompt before generating', async ({ page }) => {
  await page.addInitScript(() => {
    // Pretend WebGPU exists so the prompt check is what refuses.
    Object.defineProperty(navigator, 'gpu', { value: {}, configurable: true });
  });
  await page.goto('/tools/imagine/');
  await page.click('#tool-generate');
  await expect(page.locator('#tool-output')).toContainText('Describe the image you want first');
});

test('Generate becomes Cancel while running, and Cancel stops the run', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'gpu', { value: {}, configurable: true });
    // Stand-in model worker: reports progress, then waits to be cancelled.
    const NativeWorker = window.Worker;
    window.Worker = class extends EventTarget {
      constructor(url: string | URL, opts?: WorkerOptions) {
        super();
        if (!String(url).includes('model-worker.js')) return new NativeWorker(url, opts) as any;
      }
      postMessage(msg: any) {
        const reply = (data: any) => setTimeout(() => this.dispatchEvent(new MessageEvent('message', { data })), 0);
        if (msg.type === 'generate') reply({ type: 'progress', id: msg.id, progress: { stage: 'generate', count: 144, total: 576 } });
        if (msg.type === 'cancel') reply({ type: 'cancelled', id: msg.id });
      }
      terminate() {}
    } as any;
  });
  await page.goto('/tools/imagine/');
  await page.fill('#in-prompt', 'a lighthouse at dusk');
  await page.click('#tool-generate');
  await expect(page.locator('#tool-output')).toHaveText('Generating image… 25%');
  await expect(page.locator('#tool-generate')).toHaveText('Cancel');
  await page.click('#tool-generate');
  await expect(page.locator('#tool-output')).toHaveText('Cancelled.');
  await expect(page.locator('#tool-generate')).toHaveText('Generate image');
});
