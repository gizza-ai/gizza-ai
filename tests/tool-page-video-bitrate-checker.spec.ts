import { test, expect } from './fixtures';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tiny-128x128-audio.mp4');

async function setField(page: import('@playwright/test').Page, selector: string, value: string) {
  await page.locator(selector).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }, value);
}

async function resultJson(page: import('@playwright/test').Page) {
  await expect(page.locator('#tool-output')).toContainText('"status"', { timeout: 20_000 });
  return JSON.parse((await page.locator('#tool-output').textContent()) || '{}');
}

test('video-bitrate-checker reports exact bitrate values for a muxed MP4', async ({ page }) => {
  await page.goto('/tools/video-bitrate-checker/');
  await page.setInputFiles('#in-media', FIXTURE);

  const json = await resultJson(page);
  expect(json.status).toBe('INFO');
  expect(json.reason).toBe('not_checked');
  expect(json.container).toBe('MP4 / MOV / M4A (ISO BMFF)');
  expect(json.file_bytes).toBe(23593);
  expect(json.duration_seconds).toBe(2);
  expect(json.overall_bitrate_kbps).toBe(94.4);
  expect(json.video_bitrate_kbps).toBe(19.5);
  expect(json.audio_bitrate_kbps).toBe(64.8);
  expect(json.container_overhead_kbps).toBe(10.1);
  expect(json.streams[0].kind).toBe('video');
  expect(json.streams[0].codec).toBe('H.264 / AVC');
  expect(json.streams[0].width).toBe(128);
  expect(json.streams[0].height).toBe(128);
  expect(json.streams[1].kind).toBe('audio');
  expect(json.streams[1].codec).toBe('AAC');
});

test('video-bitrate-checker deep link applies a non-default video Mbps ceiling', async ({ page }) => {
  await page.goto('/tools/video-bitrate-checker/?max_bitrate=0.01&units=Mbps&target=video');
  await expect(page.locator('#in-max_bitrate')).toHaveValue('0.01', { timeout: 15_000 });
  await expect(page.locator('#in-units')).toHaveValue('Mbps');
  await expect(page.locator('#in-target')).toHaveValue('video');

  await page.setInputFiles('#in-media', FIXTURE);
  const json = await resultJson(page);
  expect(json.status).toBe('FAIL');
  expect(json.pass).toBe(false);
  expect(json.reason).toBe('too_high');
  expect(json.target).toBe('video');
  expect(json.units).toBe('Mbps');
  expect(json.checked_bitrate_kbps).toBe(19.5);
  expect(json.max_bitrate_kbps).toBe(10);
  expect(json.summary).toContain('FAIL');
});

test('video-bitrate-checker supports presets and all advertised target/unit values', async ({ page }) => {
  await page.goto('/tools/video-bitrate-checker/');
  await expect(page.locator('.tool-example-chip')).toHaveCount(6);

  await page.locator('.tool-example-chip', { hasText: 'Audio track at least 128 kbps' }).click();
  await expect(page.locator('#in-min_bitrate')).toHaveValue('128');
  await expect(page.locator('#in-max_bitrate')).toHaveValue('0');
  await expect(page.locator('#in-units')).toHaveValue('kbps');
  await expect(page.locator('#in-target')).toHaveValue('audio');

  await setField(page, '#in-min_bitrate', '0');
  await setField(page, '#in-max_bitrate', '0');
  await page.selectOption('#in-units', 'Mbps');
  await page.selectOption('#in-target', 'overall');
  await page.setInputFiles('#in-media', FIXTURE);
  let json = await resultJson(page);
  expect(json.status).toBe('INFO');
  expect(json.units).toBe('Mbps');
  expect(json.target).toBe('overall');

  await page.selectOption('#in-target', 'audio');
  json = await resultJson(page);
  expect(json.audio_bitrate_kbps).toBe(64.8);
});

test('video-bitrate-checker ships a runnable generated CLI example', async ({ page }) => {
  await page.goto('/tools/video-bitrate-checker/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toBe(
    "gizza tool video-bitrate-checker 'url=https://example.com/input' 'min_bitrate=0' 'max_bitrate=8000' 'units=kbps' 'target=overall'",
  );
});
