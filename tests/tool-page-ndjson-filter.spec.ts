import { test, expect } from './fixtures';

const DATA = [
  '{"name":"Alice","age":30,"city":"NYC","active":true}',
  '{"name":"Bob","age":25,"city":"LA","active":false}',
  '{"name":"Carol","age":40,"city":"NYC","active":true}',
].join('\n');

async function setField(
  page: import('@playwright/test').Page,
  id: string,
  value: string,
) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('ndjson-filter keeps matching rows and picks fields', async ({ page }) => {
  await page.goto('/tools/ndjson-filter/');
  await setField(page, '#in-data', DATA);
  await setField(page, '#in-predicate', 'city == NYC and age > 28');
  await setField(page, '#in-fields', 'name, age');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Alice', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    '{"name":"Alice","age":30}\n{"name":"Carol","age":40}\n',
  );
});

test('ndjson-filter exports active rows as CSV', async ({ page }) => {
  await page.goto('/tools/ndjson-filter/');
  await setField(page, '#in-data', DATA);
  await setField(page, '#in-predicate', 'active');
  await setField(page, '#in-fields', 'name, city, age');
  await page.selectOption('#in-format', 'csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('name,city,age', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'name,city,age\nAlice,NYC,30\nCarol,NYC,40\n',
  );
});

test('ndjson-filter deep-link pre-fills, inverts, and auto-runs', async ({ page }) => {
  const qs = new URLSearchParams({
    data: DATA,
    predicate: 'active',
    fields: 'name',
    format: 'ndjson',
    invert: 'true',
  });
  await page.goto(`/tools/ndjson-filter/?${qs.toString()}`);

  await expect(page.locator('#in-data')).toHaveValue(DATA);
  await expect(page.locator('#in-predicate')).toHaveValue('active');
  await expect(page.locator('#in-invert')).toBeChecked();

  const out = page.locator('#tool-output');
  // Invert keeps the one non-active row.
  await expect(out).toContainText('Bob', { timeout: 15_000 });
  expect(await out.textContent()).toBe('{"name":"Bob"}\n');
});
