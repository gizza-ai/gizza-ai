import { test, expect } from './fixtures';

test('fake-data-generator page renders CSV', async ({ page }) => {
  await page.goto('/tools/fake-data-generator/');
  await page.fill('#in-count', '3');
  await page.selectOption('#in-format', 'csv');
  await page.fill('#in-fields', 'full_name,email,city');
  await page.fill('#in-seed', '42');
  const out = page.locator('#tool-output');
  // header row + a synthetic email are deterministic for a fixed seed.
  await expect(out).toContainText('full_name,email,city', { timeout: 15000 });
  await expect(out).toContainText('@');
});

test('fake-data-generator page renders JSON array', async ({ page }) => {
  await page.goto('/tools/fake-data-generator/');
  await page.fill('#in-count', '2');
  await page.selectOption('#in-format', 'json');
  await page.fill('#in-fields', 'id,full_name');
  await page.fill('#in-seed', '7');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"id": "1"', { timeout: 15000 });
  await expect(out).toContainText('"full_name"');
});

test('fake-data-generator page renders SQL inserts', async ({ page }) => {
  await page.goto('/tools/fake-data-generator/');
  await page.fill('#in-count', '2');
  await page.selectOption('#in-format', 'sql');
  await page.fill('#in-fields', 'id,full_name');
  await page.fill('#in-seed', '42');
  await page.fill('#in-table', 'users');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('INSERT INTO users (id, full_name) VALUES (1,', {
    timeout: 15000,
  });
});

test('fake-data-generator CSV header toggle off', async ({ page }) => {
  await page.goto('/tools/fake-data-generator/');
  await page.fill('#in-count', '1');
  await page.selectOption('#in-format', 'csv');
  await page.fill('#in-fields', 'email');
  await page.fill('#in-seed', '3');
  // header defaults to checked; uncheck to suppress the header row.
  await page.uncheck('#in-header');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('@', { timeout: 15000 });
  await expect(out).not.toContainText('email\n');
});
