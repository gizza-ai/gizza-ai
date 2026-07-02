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
