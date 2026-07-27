import { test, expect } from './fixtures';
import path from 'node:path';

// /tools/video-trim-black-frames/ owns a custom TWO-pass ffmpeg page flow:
// blackdetect first, shared wasm trim_plan second, then a real trim encode.
// The fixture is 2s total: 0.5s black + 1.0s red + 0.5s black.

const BLACK_EDGES = path.resolve(__dirname, 'fixtures/black-edges-128x128.mp4');
const NO_BLACK_EDGES = path.resolve(__dirname, 'fixtures/redblue-64.mp4');

async function decodeVideo(page, src: string) {
  return page.evaluate(async (dataUrl: string) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('video failed to decode')), { once: true });
    });
    const b64 = dataUrl.split(',')[1] || '';
    return { w: v.videoWidth, h: v.videoHeight, dur: v.duration, bytes: atob(b64).length };
  }, src);
}

async function runFixture(page, url: string, fixture: string, fields: Record<string, string> = {}) {
  await page.goto(url);
  await page.waitForSelector('#in-file');
  for (const [name, value] of Object.entries(fields)) {
    if (name === 'ends') await page.selectOption('#in-ends', value);
    else await page.fill('#in-' + name, value);
  }
  await page.setInputFiles('#in-file', fixture);
}

test('black intro and outro are detected and trimmed with defaults', async ({ page }) => {
  await runFixture(page, '/tools/video-trim-black-frames/', BLACK_EDGES);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const r = await decodeVideo(page, src!);
  expect(r.w).toBe(128);
  expect(r.h).toBe(128);
  expect(r.dur).toBeGreaterThan(0.85);
  expect(r.dur).toBeLessThan(1.2);
  expect(r.bytes).toBeGreaterThan(500);

  await expect(page.locator('#tool-output')).toHaveText(
    'Trimmed black frames: kept 0.5s–1.5s of 2s (removed 0.5s from the start, 0.5s from the end).'
  );
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.mp4');
});

test('deep-link params prefill and ends=start keeps the trailing black', async ({ page }) => {
  await page.goto('/tools/video-trim-black-frames/?pixel_threshold=0.10&black_ratio=0.98&min_duration=0.10&ends=start');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-pixel_threshold')).toHaveValue('0.10');
  await expect(page.locator('#in-black_ratio')).toHaveValue('0.98');
  await expect(page.locator('#in-min_duration')).toHaveValue('0.10');
  await expect(page.locator('#in-ends')).toHaveValue('start');

  await page.setInputFiles('#in-file', BLACK_EDGES);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const r = await decodeVideo(page, (await media.getAttribute('src'))!);
  expect(r.dur).toBeGreaterThan(1.35);
  expect(r.dur).toBeLessThan(1.7);
  await expect(page.locator('#tool-output')).toHaveText(
    'Trimmed black frames: kept 0.5s–2s of 2s (removed 0.5s from the start, 0s from the end).'
  );
});

test('a clip with no edge black reports a friendly no-op', async ({ page }) => {
  await runFixture(page, '/tools/video-trim-black-frames/', NO_BLACK_EDGES);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('No black frames to trim', { timeout: 120_000 });
  await expect(out).not.toHaveClass(/error/);
  await expect(page.locator('#tool-output-media')).toBeHidden();
});
