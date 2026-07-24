import { test, expect } from './fixtures';
import fs from 'node:fs';
import path from 'node:path';

const mp4 = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');
const webm = path.resolve(__dirname, 'fixtures/tiny-128x128.webm');
const withAudio = path.resolve(__dirname, 'fixtures/tiny-128x128-audio.mp4');

async function outputMetadata(page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);
  const metadata = await page.evaluate(async (dataUrl) => {
    const video = document.createElement('video');
    video.muted = true;
    video.src = dataUrl;
    await new Promise((resolve, reject) => {
      video.addEventListener('loadedmetadata', resolve, { once: true });
      video.addEventListener('error', () => reject(new Error('seamless-loop output did not decode')), { once: true });
    });
    return { width: video.videoWidth, height: video.videoHeight, duration: video.duration };
  }, src!);
  return { src: src!, ...metadata };
}

async function averageFrame(page, dataUrl: string, at: number) {
  return await page.evaluate(async ({ dataUrl, at }) => {
    const video = document.createElement('video');
    video.muted = true;
    video.src = dataUrl;
    await new Promise((resolve, reject) => {
      video.addEventListener('loadedmetadata', resolve, { once: true });
      video.addEventListener('error', reject, { once: true });
    });
    video.currentTime = at;
    await new Promise((resolve) => video.addEventListener('seeked', resolve, { once: true }));
    const canvas = document.createElement('canvas');
    canvas.width = video.videoWidth;
    canvas.height = video.videoHeight;
    const context = canvas.getContext('2d')!;
    context.drawImage(video, 0, 0);
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
    const total = [0, 0, 0];
    for (let i = 0; i < pixels.length; i += 4) {
      total[0] += pixels[i];
      total[1] += pixels[i + 1];
      total[2] += pixels[i + 2];
    }
    const count = pixels.length / 4;
    return total.map((value) => value / count);
  }, { dataUrl, at });
}

test('seamless-loop-video creates a real midpoint-rotated MP4', async ({ page }) => {
  await page.goto('/tools/seamless-loop-video/');
  await page.fill('#in-duration', '2');
  await page.fill('#in-crossfade', '0.25');
  await page.selectOption('#in-audio', 'remove');
  await page.selectOption('#in-quality', 'balanced');
  await page.setInputFiles('#in-file', mp4);

  const output = await outputMetadata(page);
  expect(output.width).toBe(128);
  expect(output.height).toBe(128);
  expect(output.duration).toBeGreaterThan(1.8);
  expect(output.duration).toBeLessThan(2.3);

  // The output starts from the input midpoint. Compare average decoded pixels
  // at source 1.1 s and output 0.1 s; compression allows a small tolerance.
  const sourceData = `data:video/mp4;base64,${fs.readFileSync(mp4).toString('base64')}`;
  const sourcePixel = await averageFrame(page, sourceData, 1.1);
  const outputPixel = await averageFrame(page, output.src, 0.1);
  const meanDifference = sourcePixel.reduce(
    (sum, value, index) => sum + Math.abs(value - outputPixel[index]),
    0,
  ) / 3;
  expect(meanDifference).toBeLessThan(18);
});

test('seamless-loop-video deep link handles WebM and small quality', async ({ page }) => {
  await page.goto('/tools/seamless-loop-video/?duration=2&crossfade=0.25&audio=remove&quality=small');
  await expect(page.locator('#in-duration')).toHaveValue('2');
  await expect(page.locator('#in-crossfade')).toHaveValue('0.25');
  await expect(page.locator('#in-audio')).toHaveValue('remove');
  await expect(page.locator('#in-quality')).toHaveValue('small');
  await page.setInputFiles('#in-file', webm);
  const output = await outputMetadata(page);
  expect(output.width).toBe(128);
  expect(output.height).toBe(128);
});

test('seamless-loop-video high quality crossfades a real audio stream', async ({ page }) => {
  await page.goto('/tools/seamless-loop-video/');
  await page.fill('#in-duration', '2');
  await page.fill('#in-crossfade', '0.25');
  await page.selectOption('#in-audio', 'crossfade');
  await page.selectOption('#in-quality', 'high');
  await page.setInputFiles('#in-file', withAudio);
  const output = await outputMetadata(page);
  const decodedDuration = await page.evaluate(async (dataUrl) => {
    const bytes = await (await fetch(dataUrl)).arrayBuffer();
    const context = new AudioContext();
    const audio = await context.decodeAudioData(bytes.slice(0));
    await context.close();
    return audio.duration;
  }, output.src);
  expect(decodedDuration).toBeGreaterThan(1.8);
});

test('seamless-loop-video rejects a crossfade that reaches half the clip', async ({ page }) => {
  await page.goto('/tools/seamless-loop-video/');
  await page.fill('#in-duration', '2');
  await page.fill('#in-crossfade', '1');
  await page.setInputFiles('#in-file', mp4);
  await expect(page.locator('#tool-output')).toContainText('shorter than half', { timeout: 15_000 });
});

test('seamless-loop-video accepts the exact duration and crossfade caps', async ({ page }) => {
  await page.goto('/tools/seamless-loop-video/');
  await page.fill('#in-duration', '600');
  await page.fill('#in-crossfade', '10');
  await page.selectOption('#in-audio', 'remove');
  await page.selectOption('#in-quality', 'small');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-16x16-600s.mp4'));
  const output = await outputMetadata(page);
  expect(output.width).toBe(16);
  expect(output.height).toBe(16);
  expect(output.duration).toBeGreaterThan(590);
  expect(output.duration).toBeLessThan(605);
});
