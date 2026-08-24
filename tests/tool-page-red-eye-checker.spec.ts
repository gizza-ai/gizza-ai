import { test, expect } from './fixtures';
import path from 'node:path';

test('red-eye-checker page reports a synthetic red-eye region', async ({ page }) => {
  await page.goto('/tools/red-eye-checker/');
  await page.waitForSelector('#in-image');

  await page.selectOption('#in-sensitivity', 'high');
  await page.fill('#in-min_radius', '2');
  await page.fill('#in-max_radius', '20');
  await page.fill('#in-max_regions', '5');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/red-eye-64.png'));

  const out = page.locator('#tool-output');
  await expect(out).toContainText('candidate_count', { timeout: 20_000 });
  await expect(out).toContainText('"sensitivity": "high"');
  await expect(out).toContainText('"center_x"');
  await expect(out).toContainText('"center_y"');

  const text = await out.textContent();
  const json = JSON.parse(text || '{}');
  expect(json.candidate_count).toBeGreaterThanOrEqual(1);
  const hit = json.regions[0];
  expect(hit.center_x).toBeGreaterThanOrEqual(20);
  expect(hit.center_x).toBeLessThanOrEqual(44);
  expect(hit.center_y).toBeGreaterThanOrEqual(20);
  expect(hit.center_y).toBeLessThanOrEqual(44);
  expect(hit.confidence).toBeGreaterThan(0.2);
});

test('red-eye-checker page accepts deep-linked params before upload', async ({ page }) => {
  await page.goto('/tools/red-eye-checker/?sensitivity=low&min_radius=4&max_regions=7');
  await page.waitForSelector('#in-image');

  await expect(page.locator('#in-sensitivity')).toHaveValue('low');
  await expect(page.locator('#in-min_radius')).toHaveValue('4');
  await expect(page.locator('#in-max_regions')).toHaveValue('7');

  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/red-eye-64.png'));
  await expect(page.locator('#tool-output')).toContainText('"sensitivity": "low"', { timeout: 20_000 });
});
