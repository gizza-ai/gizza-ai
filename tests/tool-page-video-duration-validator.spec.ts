import { test, expect } from './fixtures';
import path from 'node:path';

const THREE_SECONDS = path.resolve(__dirname, 'fixtures/title-black-3s.mp4');
const TWO_SECONDS = path.resolve(__dirname, 'fixtures/redblue-64.mp4');
const TEN_MINUTES = path.resolve(__dirname, 'fixtures/tiny-16x16-600s.mp4');

async function setField(page: import('@playwright/test').Page, selector: string, value: string) {
  await page.locator(selector).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }, value);
}

async function resultJson(page: import('@playwright/test').Page) {
  await expect(page.locator('#tool-output')).toContainText('"status"', { timeout: 15_000 });
  return JSON.parse((await page.locator('#tool-output').textContent()) || '{}');
}

test('video-duration-validator passes a 3s clip inside the target window', async ({ page }) => {
  await page.goto('/tools/video-duration-validator/');
  await setField(page, '#in-target_seconds', '3');
  await setField(page, '#in-tolerance_seconds', '0.1');
  await page.selectOption('#in-mode', 'within');
  await page.setInputFiles('#in-video', THREE_SECONDS);

  const json = await resultJson(page);
  expect(json.status).toBe('PASS');
  expect(json.pass).toBe(true);
  expect(json.reason).toBe('ok');
  expect(json.mode).toBe('within');
  expect(json.actual_seconds).toBeCloseTo(3, 3);
  expect(json.allowed_min_seconds).toBe(2.9);
  expect(json.allowed_max_seconds).toBe(3.1);
  expect(json.summary).toContain('PASS');
});

test('video-duration-validator deep link flags a too-short clip', async ({ page }) => {
  await page.goto('/tools/video-duration-validator/?target_seconds=3&tolerance_seconds=0.25&mode=within');
  await expect(page.locator('#in-target_seconds')).toHaveValue('3', { timeout: 15_000 });
  await expect(page.locator('#in-tolerance_seconds')).toHaveValue('0.25');
  await expect(page.locator('#in-mode')).toHaveValue('within');
  await page.setInputFiles('#in-video', TWO_SECONDS);

  const json = await resultJson(page);
  expect(json.status).toBe('FAIL');
  expect(json.pass).toBe(false);
  expect(json.reason).toBe('too_short');
  expect(json.actual_seconds).toBeCloseTo(2, 3);
  expect(json.delta_seconds).toBeCloseTo(-1, 3);
  expect(json.overshoot_seconds).toBeCloseTo(0.75, 3);
});

test('video-duration-validator supports max and min modes', async ({ page }) => {
  await page.goto('/tools/video-duration-validator/');
  await setField(page, '#in-target_seconds', '2');
  await setField(page, '#in-tolerance_seconds', '0');
  await page.selectOption('#in-mode', 'max');
  await page.setInputFiles('#in-video', THREE_SECONDS);
  let json = await resultJson(page);
  expect(json.status).toBe('FAIL');
  expect(json.reason).toBe('too_long');
  expect(json.allowed_max_seconds).toBe(2);

  await page.selectOption('#in-mode', 'min');
  json = await resultJson(page);
  expect(json.status).toBe('PASS');
  expect(json.allowed_min_seconds).toBe(2);
  expect(json.allowed_max_seconds).toBeUndefined();
});

test('video-duration-validator accepts the exact 24-hour target cap boundary', async ({ page }) => {
  await page.goto('/tools/video-duration-validator/');
  await setField(page, '#in-target_seconds', '86400');
  await setField(page, '#in-tolerance_seconds', '0');
  await page.selectOption('#in-mode', 'max');
  await page.setInputFiles('#in-video', TEN_MINUTES);

  const json = await resultJson(page);
  expect(json.status).toBe('PASS');
  expect(json.target_seconds).toBe(86400);
  expect(json.actual_seconds).toBeCloseTo(600, 3);
});

test('video-duration-validator ships preset chips and a runnable CLI example', async ({ page }) => {
  await page.goto('/tools/video-duration-validator/');
  await expect(page.locator('.tool-example-chip')).toHaveCount(6);
  await page.locator('.tool-example-chip', { hasText: 'Short-form cap — 60s max' }).click();
  await expect(page.locator('#in-target_seconds')).toHaveValue('60');
  await expect(page.locator('#in-mode')).toHaveValue('max');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toBe(
    "gizza tool video-duration-validator 'url=https://example.com/input' 'target_seconds=30' 'tolerance_seconds=0.5' 'mode=within'",
  );
});
