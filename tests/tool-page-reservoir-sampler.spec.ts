import { test, expect } from './fixtures';

const LOGS = 'GET /home 200\nGET /pricing 200\nPOST /signup 500\nGET /docs 200\nGET /home 304\nPOST /login 401\nGET /blog 200\nGET /home 200\nDELETE /account 204\nGET /pricing 500';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('reservoir-sampler draws a deterministic sample', async ({ page }) => {
  await page.goto('/tools/reservoir-sampler/');
  await setField(page, '#in-data', LOGS);
  await setField(page, '#in-k', '4');
  await page.selectOption('#in-algorithm', 'l');
  await setField(page, '#in-seed', '42');

  await expect(page.locator('#tool-output')).toHaveText(
    'POST /signup 500\nGET /docs 200\nPOST /login 401\nGET /blog 200',
    { timeout: 15_000 },
  );
});

test('reservoir-sampler honors deep-linked JSON parameters', async ({ page }) => {
  const params = new URLSearchParams({
    data: LOGS,
    k: '2',
    algorithm: 'l',
    seed: '1',
    skip_empty: 'true',
    header: 'false',
    order: 'input',
    format: 'json',
    stats: 'false',
  });
  await page.goto(`/tools/reservoir-sampler/?${params.toString()}`);

  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#in-seed')).toHaveValue('1');
  await expect(page.locator('#tool-output')).toHaveText(
    '[{"line":5,"text":"GET /home 304"},{"line":10,"text":"GET /pricing 500"}]',
    { timeout: 15_000 },
  );
});

test('reservoir-sampler covers algorithm, order, checkbox and k boundary controls', async ({ page }) => {
  await page.goto('/tools/reservoir-sampler/');
  await setField(page, '#in-data', LOGS);
  await setField(page, '#in-k', '3');
  await page.selectOption('#in-algorithm', 'r');
  await setField(page, '#in-seed', '7');
  await page.selectOption('#in-order', 'reservoir');
  await page.selectOption('#in-format', 'numbered');
  await page.check('#in-stats');

  await expect(page.locator('#tool-output')).toHaveText(
    '7\tGET /blog 200\n4\tGET /docs 200\n6\tPOST /login 401\n\n# sampled 3 of 10 records | p = 0.3000 | algorithm R | seed 7',
    { timeout: 15_000 },
  );

  await setField(page, '#in-data', 'name\nada\n\nbeau\ncy');
  await setField(page, '#in-k', '2');
  await page.selectOption('#in-algorithm', 'l');
  await page.selectOption('#in-order', 'input');
  await page.selectOption('#in-format', 'lines');
  await page.check('#in-header');
  await page.uncheck('#in-skip_empty');
  await page.uncheck('#in-stats');
  await expect(page.locator('#tool-output')).toContainText('name', { timeout: 15_000 });

  await setField(page, '#in-k', '1000001');
  await expect(page.locator('#tool-output')).toContainText('k must be between 1 and 1000000', { timeout: 15_000 });
});
