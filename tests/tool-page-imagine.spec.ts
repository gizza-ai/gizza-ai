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

// Stand-in model worker driven from the test: requests are recorded and the
// test decides when (and whether) the worker answers.
async function scriptedWorker(page: import('@playwright/test').Page) {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'gpu', { value: {}, configurable: true });
    const NativeWorker = window.Worker;
    const w = window as any;
    w.__requests = [];
    window.Worker = class extends EventTarget {
      constructor(url: string | URL, opts?: WorkerOptions) {
        super();
        if (!String(url).includes('model-worker.js')) return new NativeWorker(url, opts) as any;
        w.__reply = (data: any) => this.dispatchEvent(new MessageEvent('message', { data }));
      }
      postMessage(msg: any) {
        w.__requests.push({ type: msg.type, id: msg.id });
      }
      terminate() {}
    } as any;
  });
}

// A 1x1 PNG as the "generated" image.
const PNG_1X1 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';

async function finishWithImage(page: import('@playwright/test').Page, id: number) {
  await page.evaluate(({ id, b64 }) => {
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    (window as any).__reply({ type: 'result', id, blob: new Blob([bytes], { type: 'image/png' }), filename: 'generated.png', backend: 'webgpu' });
  }, { id, b64: PNG_1X1 });
}

// The id of the latest generate request; waits for it (the page posts it
// after lazily importing the model client).
async function lastGenerateId(page: import('@playwright/test').Page, count = 1): Promise<number> {
  const generateIds = () =>
    page.evaluate(() => (window as any).__requests.filter((r: any) => r.type === 'generate').map((r: any) => r.id));
  await expect.poll(async () => (await generateIds()).length).toBe(count);
  return (await generateIds()).at(-1);
}

test('a new Generate hides the previous image until its own result arrives', async ({ page }) => {
  await scriptedWorker(page);
  await page.goto('/tools/imagine/');
  await page.fill('#in-prompt', 'a lighthouse at dusk');
  await page.click('#tool-generate');
  await finishWithImage(page, await lastGenerateId(page));
  await expect(page.locator('#tool-output-media')).toBeVisible();
  await expect(page.locator('#tool-output-download')).toBeVisible();

  await page.click('#tool-generate');
  await expect(page.locator('#tool-generate')).toHaveText('Cancel');
  await expect(page.locator('#tool-output-media')).toBeHidden();
  await expect(page.locator('#tool-output-download')).toBeHidden();
  await expect(page.locator('#tool-copy-image')).toBeHidden();
});

test('Reset cancels an in-flight generation so its image never reappears', async ({ page }) => {
  await scriptedWorker(page);
  await page.goto('/tools/imagine/');
  await page.fill('#in-prompt', 'a lighthouse at dusk');
  await page.click('#tool-generate');
  const id = await lastGenerateId(page);
  await page.click('#tool-reset');
  await expect.poll(() => page.evaluate(() => (window as any).__requests.map((r: any) => r.type))).toContain('cancel');
  // The worker finished before it read the cancel: the result is dropped.
  await finishWithImage(page, id);
  await expect(page.locator('#tool-generate')).toHaveText('Generate image');
  await expect(page.locator('#tool-output-media')).toBeHidden();
  await expect(page.locator('#tool-output')).toHaveText('');
  await expect(page.locator('#in-prompt')).toHaveValue('');
});
