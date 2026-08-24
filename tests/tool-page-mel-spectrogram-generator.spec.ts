import { test, expect } from './fixtures';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.wav');

async function pngInfo(page) {
  return await page.locator('#tool-output-media').evaluate(async (img) => {
    const el = img as HTMLImageElement;
    const res = await fetch(el.src);
    const bytes = new Uint8Array(await res.arrayBuffer());
    return {
      src: el.src.slice(0, 32),
      width: el.naturalWidth,
      height: el.naturalHeight,
      signature: Array.from(bytes.slice(0, 8)),
      bytes: bytes.length,
    };
  });
}

test('mel-spectrogram-generator page renders a real PNG from uploaded audio', async ({ page }) => {
  await page.goto('/tools/mel-spectrogram-generator/');
  await page.waitForSelector('#in-input');
  await page.setInputFiles('#in-input', FIXTURE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('1000x400 PNG', { timeout: 45_000 });
  await expect(out).toContainText('Mel: 128 bands');
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible();

  const info = await pngInfo(page);
  expect(info.src.startsWith('data:image/png;base64,')).toBeTruthy();
  expect(info.width).toBe(1000);
  expect(info.height).toBe(400);
  expect(info.signature).toEqual([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  expect(info.bytes).toBeGreaterThan(1000);
});

test('mel-spectrogram-generator deep link applies compact speech parameters', async ({ page }) => {
  await page.goto('/tools/mel-spectrogram-generator/?n_fft=1024&hop_length=256&n_mels=80&fmax=8000&mel_scale=htk&window=hamming&scale=power&top_db=60&db_reference=full_scale&colormap=viridis&width=320&height=160&channel=left&center=false&resample_hz=16000');
  await page.waitForSelector('#in-input');

  await expect(page.locator('#in-n_fft')).toHaveValue('1024');
  await expect(page.locator('#in-hop_length')).toHaveValue('256');
  await expect(page.locator('#in-n_mels')).toHaveValue('80');
  await expect(page.locator('#in-mel_scale')).toHaveValue('htk');
  await expect(page.locator('#in-window')).toHaveValue('hamming');
  await expect(page.locator('#in-scale')).toHaveValue('power');
  await expect(page.locator('#in-db_reference')).toHaveValue('full_scale');
  await expect(page.locator('#in-colormap')).toHaveValue('viridis');
  await expect(page.locator('#in-center')).not.toBeChecked();

  await page.setInputFiles('#in-input', FIXTURE);
  await expect(page.locator('#tool-output')).toContainText('320x160 PNG', { timeout: 45_000 });
  await expect(page.locator('#tool-output')).toContainText('80 bands');
  const info = await pngInfo(page);
  expect(info.width).toBe(320);
  expect(info.height).toBe(160);
});
