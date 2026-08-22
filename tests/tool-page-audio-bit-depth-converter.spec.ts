import { test, expect } from './fixtures';
import path from 'node:path';

const WAV_FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.wav');
const MP3_FIXTURE = path.resolve(__dirname, 'fixtures/tone-quiet-3s.mp3');

async function decodeAudio(page, src: string): Promise<{ duration: number }> {
  return page.evaluate(async (dataUrl) => {
    const a = document.createElement('audio');
    a.preload = 'metadata';
    await new Promise<void>((res, rej) => {
      a.onloadedmetadata = () => res();
      a.onerror = () => rej(new Error('audio decode failed'));
      a.src = dataUrl;
    });
    return { duration: a.duration };
  }, src);
}

test('audio-bit-depth-converter converts WAV to default 16-bit dithered WAV output', async ({ page }) => {
  await page.goto('/tools/audio-bit-depth-converter/');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-bit_depth')).toHaveValue('16');
  await expect(page.locator('#in-dither')).toHaveValue('triangular');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await expect(page.locator('#in-keep_metadata')).toBeChecked();

  await page.setInputFiles('#in-audio', WAV_FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/wav/);
  const meta = await decodeAudio(page, src!);
  expect(meta.duration).toBeGreaterThan(2.5);
  expect(meta.duration).toBeLessThan(3.5);
});

test('audio-bit-depth-converter deep link exercises 24-bit FLAC and metadata stripping', async ({ page }) => {
  await page.goto('/tools/audio-bit-depth-converter/?bit_depth=24&dither=shibata&format=flac&keep_metadata=false');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-bit_depth')).toHaveValue('24', { timeout: 15_000 });
  await expect(page.locator('#in-dither')).toHaveValue('shibata');
  await expect(page.locator('#in-format')).toHaveValue('flac');
  await expect(page.locator('#in-keep_metadata')).not.toBeChecked();

  await page.setInputFiles('#in-audio', MP3_FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/flac/);
  const meta = await decodeAudio(page, src!);
  expect(meta.duration).toBeGreaterThan(2.5);
  expect(meta.duration).toBeLessThan(3.5);
});

test('audio-bit-depth-converter page ships runnable CLI, labels, and preset chips', async ({ page }) => {
  await page.goto('/tools/audio-bit-depth-converter/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toBe(
    "gizza tool audio-bit-depth-converter 'url=https://example.com/input' 'bit_depth=16' 'dither=triangular' 'format=wav' 'keep_metadata=true'"
  );
  await expect(page.locator('#in-bit_depth option[value="32f"]')).toHaveText('32-bit float — DAW interchange');
  await expect(page.locator('#in-dither option[value="shibata"]')).toHaveText('Shibata — noise shaped');
  await expect(page.locator('#in-format option[value="flac"]')).toHaveText('FLAC — compressed, 16 & 24-bit only');
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
});
