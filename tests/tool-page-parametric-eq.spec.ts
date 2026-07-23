import { test, expect } from './fixtures';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3');

async function decodedDuration(page, dataUrl: string): Promise<number> {
  return page.evaluate(async (src) => {
    const buf = await (await fetch(src)).arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    return decoded.duration;
  }, dataUrl);
}

test('parametric-eq renders a real mp3 output for a vocal-presence boost', async ({ page }) => {
  await page.goto('/tools/parametric-eq/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-band2_freq', '3000');
  await page.fill('#in-band2_gain', '4');
  await page.fill('#in-band2_q', '1.5');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodedDuration(page, src!);
  expect(duration).toBeGreaterThan(2.8);
  expect(duration).toBeLessThan(3.4);
});

test('parametric-eq deep-link fills all band values and wav output', async ({ page }) => {
  await page.goto('/tools/parametric-eq/?band1_freq=250&band1_gain=-5&band1_q=1.2&band2_freq=3000&band2_gain=4&band2_q=1.5&band3_freq=10000&band3_gain=3&band3_q=0.7&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-band1_freq')).toHaveValue('250', { timeout: 15_000 });
  await expect(page.locator('#in-band1_gain')).toHaveValue('-5');
  await expect(page.locator('#in-band1_q')).toHaveValue('1.2');
  await expect(page.locator('#in-band2_freq')).toHaveValue('3000');
  await expect(page.locator('#in-band2_gain')).toHaveValue('4');
  await expect(page.locator('#in-band2_q')).toHaveValue('1.5');
  await expect(page.locator('#in-band3_freq')).toHaveValue('10000');
  await expect(page.locator('#in-band3_gain')).toHaveValue('3');
  await expect(page.locator('#in-band3_q')).toHaveValue('0.7');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/wav/);
});

test('parametric-eq reports an all-zero EQ as a no-op error', async ({ page }) => {
  await page.goto('/tools/parametric-eq/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  await expect(page.locator('#tool-output')).toContainText('nothing to change', { timeout: 90_000 });
});
