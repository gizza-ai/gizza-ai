import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// Shared waveform component (site/tool-audio.js): every audio-input ffmpeg
// tool page renders an interactive waveform after upload. audio-convert is
// the UNBOUND case: selection is audition-only and must never write fields.
// NOTE: two .tool-wf containers exist per page (input + output) — always
// disambiguate with .first()/.nth(1) to satisfy Playwright strict mode.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3'); // 3.03 s tone

test('audio-convert page renders a non-blank waveform after upload', async ({ page }) => {
  await page.goto('/tools/audio-convert/');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('.tool-wf').first()).toBeHidden();
  await page.setInputFiles('#in-audio', FIXTURE);
  await expect(page.locator('.tool-wf-wave').first()).toBeVisible({ timeout: 15_000 });
  // Canvas must actually contain drawn waveform pixels, not be blank.
  const drawn = await page.evaluate(() => {
    const c = document.querySelector('.tool-wf-canvas') as HTMLCanvasElement;
    const g = c.getContext('2d')!;
    const d = g.getImageData(0, 0, c.width, c.height).data;
    let painted = 0;
    for (let i = 3; i < d.length; i += 4) if (d[i] > 0) painted++;
    return painted;
  });
  expect(drawn).toBeGreaterThan(500);
});

test('audio-convert waveform plays and dragging writes no fields', async ({ page }) => {
  await page.goto('/tools/audio-convert/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  const wave = page.locator('.tool-wf-wave').first();
  await expect(wave).toBeVisible({ timeout: 15_000 });
  const bitrateBefore = await page.locator('#in-bitrate').inputValue();

  // Drag an audition selection across the middle of the waveform.
  const box = (await wave.boundingBox())!;
  const y = box.y + box.height / 2;
  await page.mouse.move(box.x + box.width * 0.25, y);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.5, y, { steps: 8 });
  await page.mouse.up();
  await expect(page.locator('.tool-wf-playsel').first()).toBeVisible();
  // Unbound tool: no field values changed by the drag.
  expect(await page.locator('#in-bitrate').inputValue()).toBe(bitrateBefore);

  // Play advances the underlying audio clock.
  await page.locator('.tool-wf-play').first().click();
  await page.waitForTimeout(400);
  const t = await page.evaluate(
    () => (document.querySelector('.tool-wf-time') as HTMLElement).textContent
  );
  expect(t).not.toContain('0:00.0 /');
});

async function decodeDurationOfDataUrl(page: Page, src: string): Promise<number> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    return decoded.duration;
  }, src);
}

test('trim-audio drag-selection writes start/end and trims to the selection', async ({ page }) => {
  await page.goto('/tools/trim-audio/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  const wave = page.locator('.tool-wf-wave').first();
  await expect(wave).toBeVisible({ timeout: 15_000 });

  // Drag 25% → 50% of a 3.03 s tone ⇒ start ≈ 0.76, end ≈ 1.52.
  const box = (await wave.boundingBox())!;
  const y = box.y + box.height / 2;
  await page.mouse.move(box.x + box.width * 0.25, y);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.5, y, { steps: 8 });
  await page.mouse.up();

  const start = parseFloat(await page.locator('#in-start').inputValue());
  const end = parseFloat(await page.locator('#in-end').inputValue());
  expect(start).toBeGreaterThan(0.55);
  expect(start).toBeLessThan(0.95);
  expect(end).toBeGreaterThan(1.3);
  expect(end).toBeLessThan(1.75);

  // The commit fired one run; the trimmed output matches the selection length.
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const dur = await decodeDurationOfDataUrl(page, src!);
  expect(Math.abs(dur - (end - start))).toBeLessThan(0.2);
});

test('trim-audio typing start/end moves the selection highlight', async ({ page }) => {
  await page.goto('/tools/trim-audio/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  await expect(page.locator('.tool-wf-wave').first()).toBeVisible({ timeout: 15_000 });
  await page.locator('#in-start').fill('1');
  await page.locator('#in-end').fill('2');
  // The bar's selection readout mirrors the typed values (0:01.0–0:02.0 (1.0s)).
  await expect(page.locator('.tool-wf-time').first()).toContainText('0:01.0–0:02.0');
});
