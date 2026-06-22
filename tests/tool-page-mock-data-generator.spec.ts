import { test, expect } from './fixtures';

test('mock-data-generator page generates a JSON array', async ({ page }) => {
  await page.goto('/tools/mock-data-generator/');
  await page.fill('#in-schema', 'id:int(1..100), name:name, email:email');
  await page.fill('#in-count', '3');
  await page.fill('#in-seed', '42');
  const out = page.locator('#tool-output');
  // count=3 → a JSON array; the keys + a synthetic email are deterministic.
  await expect(out).toContainText('"name"', { timeout: 15000 });
  await expect(out).toContainText('@');
  const text = await out.textContent();
  const parsed = JSON.parse(text!.trim());
  expect(Array.isArray(parsed)).toBe(true);
  expect(parsed.length).toBe(3);
});

test('mock-data-generator page renders nested objects and arrays', async ({ page }) => {
  await page.goto('/tools/mock-data-generator/');
  await page.fill('#in-schema', 'user:{first:first_name, last:last_name}, tags:word[3]');
  await page.fill('#in-count', '1');
  await page.fill('#in-seed', '7');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"user"', { timeout: 15000 });
  const parsed = JSON.parse((await out.textContent())!.trim());
  expect(typeof parsed.user).toBe('object');
  expect(typeof parsed.user.first).toBe('string');
  expect(Array.isArray(parsed.tags)).toBe(true);
  expect(parsed.tags.length).toBe(3);
});

test('mock-data-generator compact (pretty off) is single line', async ({ page }) => {
  await page.goto('/tools/mock-data-generator/');
  await page.fill('#in-schema', 'a:int, b:bool');
  await page.fill('#in-count', '1');
  await page.fill('#in-seed', '5');
  // pretty defaults to checked; uncheck for compact JSON.
  await page.uncheck('#in-pretty');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"a"', { timeout: 15000 });
  const text = (await out.textContent())!.trim();
  expect(text).not.toContain('\n');
});

test('mock-data-generator reports an invalid type', async ({ page }) => {
  await page.goto('/tools/mock-data-generator/');
  await page.fill('#in-schema', 'x:frobnicate');
  await page.fill('#in-count', '1');
  await page.fill('#in-seed', '1');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('unknown type', { timeout: 15000 });
});
