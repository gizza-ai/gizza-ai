import { test, expect } from './fixtures';
import * as fs from 'fs';
import * as path from 'path';

test('AI background-removal page wires a local worker result', async ({ page }) => {
  await page.goto('/tools/image-background-remove-ai/');
  await page.evaluate(() => {
    class FakeModelWorker extends EventTarget {
      postMessage(message: any) {
        setTimeout(() => {
          this.dispatchEvent(new MessageEvent('message', {
            data: {
              type: 'progress',
              id: message.id,
              progress: { stage: 'inference', device: 'wasm' },
            },
          }));
          this.dispatchEvent(new MessageEvent('message', {
            data: {
              type: 'result',
              id: message.id,
              blob: message.file,
              filename: 'background-removed.png',
              backend: 'wasm',
            },
          }));
        }, 0);
      }
      terminate() {}
    }
    Object.defineProperty(window, 'Worker', { configurable: true, value: FakeModelWorker });
  });

  await expect(page.locator('.tool-model-note')).toContainText('never uploaded for inference');
  const dropzone = page.locator('.tool-file-dropzone');
  await expect(dropzone).toBeVisible();
  await expect(page.locator('.tool-output-label')).toBeHidden();
  await expect(page.locator('#tool-output')).toBeHidden();

  const fileBase64 = fs.readFileSync(path.resolve(__dirname, 'fixtures/red-2x2.png')).toString('base64');
  await page.evaluate((base64) => {
    const bytes = Uint8Array.from(atob(base64), (char) => char.charCodeAt(0));
    const transfer = new DataTransfer();
    transfer.items.add(new File([bytes], 'dropped.png', { type: 'image/png' }));
    document.querySelector('.tool-file-dropzone')!.dispatchEvent(new DragEvent('drop', {
      bubbles: true,
      cancelable: true,
      dataTransfer: transfer,
    }));
  }, fileBase64);
  await expect(dropzone).toHaveClass(/has-file/);
  await expect(page.locator('.tool-file-dropzone-title')).toHaveText('dropped.png');
  await expect(page.locator('.tool-output-label')).toBeVisible();
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible();
  expect(await media.getAttribute('src')).toMatch(/^blob:/);
  await expect(media).toHaveAttribute('alt', 'Transparent cutout');
  const centers = await page.evaluate(() => {
    const mediaBox = document.querySelector('#tool-output-media')!.getBoundingClientRect();
    const widgetBox = document.querySelector('.tool-widget')!.getBoundingClientRect();
    return {
      media: mediaBox.left + mediaBox.width / 2,
      widget: widgetBox.left + widgetBox.width / 2,
    };
  });
  expect(Math.abs(centers.media - centers.widget)).toBeLessThan(2);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'background-removed.png');
  await expect(page.locator('#tool-output')).toContainText('Finished locally with wasm');

  await page.evaluate(() => {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { write: async () => { throw new Error('denied'); } },
    });
    Object.defineProperty(window, 'ClipboardItem', {
      configurable: true,
      value: class ClipboardItem {},
    });
  });
  const copy = page.locator('#tool-copy-image');
  await copy.click();
  await expect(copy).toHaveClass(/copy-failed/);
  await expect(copy).toHaveText("Couldn't copy");
  await expect(copy).toHaveAttribute('title', 'Copy failed. Use Download instead.');
});

test('AI background-removal page runs the pinned model', async ({ page }, testInfo) => {
  test.skip(!process.env.RUN_MODEL_E2E, 'Set RUN_MODEL_E2E=1 for the network-backed model smoke test');
  const imageInputs: Array<{ label: string; file: string | { name: string; mimeType: string; buffer: Buffer } }> = [];
  if (process.env.MODEL_E2E_IMAGES) {
    for (const imagePath of JSON.parse(process.env.MODEL_E2E_IMAGES) as string[]) {
      imageInputs.push({ label: path.basename(imagePath), file: imagePath });
    }
  } else {
    const examples = [
      ['horse.jpg', 'image/jpeg'],
      ['lamp2_meitu_1.jpg', 'image/jpeg'],
      ['whisk.png', 'image/png'],
      ['bike.jpg', 'image/jpeg'],
      ['girl.png', 'image/png'],
    ] as const;
    for (const [name, mimeType] of examples) {
      const response = await fetch(
        `https://raw.githubusercontent.com/xuebinqin/U-2-Net/master/test_data/test_images/${name}`,
      );
      expect(response.ok).toBe(true);
      imageInputs.push({
        label: `official-u2net-${name}`,
        file: {
          name: `official-u2net-${name}`,
          mimeType,
          buffer: Buffer.from(await response.arrayBuffer()),
        },
      });
    }
  }
  test.setTimeout(180_000 + imageInputs.length * 120_000);

  await page.goto('/tools/image-background-remove-ai/');
  const status = page.locator('#tool-output');
  const media = page.locator('#tool-output-media');
  const downloadLink = page.locator('#tool-output-download');

  for (const input of imageInputs) {
    const previousSrc = await media.getAttribute('src');
    await page.setInputFiles('#in-image', input.file);
    await expect.poll(() => media.getAttribute('src'), { timeout: 270_000 }).not.toBe(previousSrc);
    await expect(status).toContainText(/Finished locally with (wasm|webgpu)/);
    await expect(media).toBeVisible();
    await expect(downloadLink).toHaveAttribute('download', 'background-removed.png');

    const result = await media.evaluate(async (image: HTMLImageElement) => {
      await image.decode();
      const response = await fetch(image.src);
      const blob = await response.blob();
      const canvas = document.createElement('canvas');
      canvas.width = image.naturalWidth;
      canvas.height = image.naturalHeight;
      const context = canvas.getContext('2d', { willReadFrequently: true });
      context!.drawImage(image, 0, 0);
      const pixels = context!.getImageData(0, 0, canvas.width, canvas.height).data;
      const pixelCount = canvas.width * canvas.height;
      let alphaSum = 0;
      let foregroundPixels = 0;
      let backgroundPixels = 0;
      let opaquePixels = 0;
      let transparentPixels = 0;
      for (let index = 3; index < pixels.length; index += 4) {
        const alpha = pixels[index];
        alphaSum += alpha;
        if (alpha >= 16) foregroundPixels += 1;
        if (alpha <= 239) backgroundPixels += 1;
        if (alpha >= 239) opaquePixels += 1;
        if (alpha <= 16) transparentPixels += 1;
      }

      const mapWidth = 40;
      const mapHeight = Math.max(8, Math.round(mapWidth * canvas.height / canvas.width / 2));
      const ramp = ' .:-=+*#%@';
      const alphaMap: string[] = [];
      for (let mapY = 0; mapY < mapHeight; mapY += 1) {
        let line = '';
        for (let mapX = 0; mapX < mapWidth; mapX += 1) {
          const pixelX = Math.min(canvas.width - 1, Math.floor((mapX + 0.5) * canvas.width / mapWidth));
          const pixelY = Math.min(canvas.height - 1, Math.floor((mapY + 0.5) * canvas.height / mapHeight));
          const alpha = pixels[(pixelY * canvas.width + pixelX) * 4 + 3];
          line += ramp[Math.round(alpha / 255 * (ramp.length - 1))];
        }
        alphaMap.push(line);
      }
      return {
        type: blob.type,
        size: blob.size,
        width: image.naturalWidth,
        height: image.naturalHeight,
        meanAlpha: alphaSum / pixelCount / 255,
        foregroundRatio: foregroundPixels / pixelCount,
        backgroundRatio: backgroundPixels / pixelCount,
        opaqueRatio: opaquePixels / pixelCount,
        transparentRatio: transparentPixels / pixelCount,
        alphaMap: alphaMap.join('\n'),
      };
    });
    const { alphaMap, ...metrics } = result;
    console.log(`background-removal metrics ${input.label}: ${JSON.stringify(metrics)}`);
    console.log(`background-removal alpha map ${input.label}:\n${alphaMap}`);
    expect(result.type).toBe('image/png');
    expect(result.size).toBeGreaterThan(100);
    expect(result.foregroundRatio).toBeGreaterThan(0.005);
    expect(result.backgroundRatio).toBeGreaterThan(0.005);
    expect(result.opaqueRatio).toBeGreaterThan(0.001);
    expect(result.transparentRatio).toBeGreaterThan(0.001);

    const artifactName = `cutout-${input.label.replace(/[^a-z0-9.-]+/gi, '-')}.png`;
    const artifactPath = testInfo.outputPath(artifactName);
    const downloadPromise = page.waitForEvent('download');
    await downloadLink.click();
    const download = await downloadPromise;
    await download.saveAs(artifactPath);
    await testInfo.attach(artifactName, { path: artifactPath, contentType: 'image/png' });
  }
});

test('AI background-removal page rejects an empty foreground mask', async ({ page }) => {
  await page.goto('/tools/image-background-remove-ai/');
  await page.evaluate(() => {
    class EmptyMaskModelWorker extends EventTarget {
      postMessage(message: any) {
        setTimeout(() => {
          this.dispatchEvent(new MessageEvent('message', {
            data: {
              type: 'result',
              id: message.id,
              blob: message.file,
              metrics: {
                meanAlpha: 0,
                foregroundRatio: 0,
                backgroundRatio: 1,
              },
              filename: 'background-removed.png',
              backend: 'wasm',
            },
          }));
        }, 0);
      }
      terminate() {}
    }
    Object.defineProperty(window, 'Worker', { configurable: true, value: EmptyMaskModelWorker });
  });
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/photo-320.jpg'));

  await expect(page.locator('#tool-output')).toContainText('No foreground subject was detected');
  await expect(page.locator('#tool-output-media')).toBeHidden();
  await expect(page.locator('#tool-output-download')).toBeHidden();
});
