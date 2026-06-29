import { test, expect } from './fixtures';

// /tools/percentage-calculator/ answers the five everyday percentage questions
// in-browser (pure wasm) and renders the result as pretty-printed JSON.

test('percentage-calculator computes percent_of', async ({ page }) => {
  await page.goto('/tools/percentage-calculator/');
  await page.selectOption('#in-mode', 'percent_of');
  await page.fill('#in-percent', '15');
  await page.fill('#in-base', '200');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"mode": "percent_of"', { timeout: 15000 });
  await expect(out).toContainText('"value": 30.0');
  await expect(out).toContainText('15% of 200 = 30');
});

test('percentage-calculator computes what_percent', async ({ page }) => {
  await page.goto('/tools/percentage-calculator/');
  await page.selectOption('#in-mode', 'what_percent');
  await page.fill('#in-part', '30');
  await page.fill('#in-whole', '200');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"mode": "what_percent"', { timeout: 15000 });
  await expect(out).toContainText('"value": 15.0');
  await expect(out).toContainText('30 is 15% of 200');
});

test('percentage-calculator computes percent change', async ({ page }) => {
  await page.goto('/tools/percentage-calculator/?mode=change&from=200&to=230');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"mode": "change"', { timeout: 15000 });
  await expect(out).toContainText('"value": 15.0');
  await expect(out).toContainText('"name": "absolute_change"');
  await expect(out).toContainText('15% increase');
});

test('percentage-calculator applies a percent change', async ({ page }) => {
  await page.goto('/tools/percentage-calculator/');
  await page.selectOption('#in-mode', 'apply_change');
  await page.fill('#in-base', '200');
  await page.fill('#in-percent', '15');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"mode": "apply_change"', { timeout: 15000 });
  await expect(out).toContainText('"value": 230.0');
  await expect(out).toContainText('200 increased by 15% = 230');
});

test('percentage-calculator computes percent_of_total with remaining', async ({ page }) => {
  await page.goto('/tools/percentage-calculator/?mode=percent_of_total&value=30&total=200');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"mode": "percent_of_total"', { timeout: 15000 });
  await expect(out).toContainText('"value": 15.0');
  await expect(out).toContainText('"name": "remaining"');
  await expect(out).toContainText('"value": 85.0');
});

test('percentage-calculator deep-links mode + numbers via query params', async ({ page }) => {
  await page.goto('/tools/percentage-calculator/?mode=change&from=50&to=40');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"mode": "change"', { timeout: 15000 });
  await expect(out).toContainText('"value": -20.0');
  await expect(out).toContainText('decrease');
});

test('percentage-calculator shows a prompt before any number is entered', async ({ page }) => {
  await page.goto('/tools/percentage-calculator/');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Enter the numbers', { timeout: 15000 });
});
