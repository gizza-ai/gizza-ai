import { test, expect } from './fixtures';

const CSV = 'name,weight\nAlice,5\nBob,1\nCarol,3\nDan,1';

test('weighted-random-sampler returns deterministic seeded CSV sample', async ({ page }) => {
  await page.goto('/tools/weighted-random-sampler/');
  await page.fill('#in-data', CSV);
  await page.selectOption('#in-format', 'csv');
  await page.fill('#in-weight_field', 'weight');
  await page.fill('#in-n', '2');
  await page.fill('#in-seed', '42');

  await expect(page.locator('#tool-output')).toHaveText('name,weight\nAlice,5\nCarol,3', {
    timeout: 15000,
  });
});

test('weighted-random-sampler supports replacement and tab-delimited no-header CSV', async ({ page }) => {
  await page.goto('/tools/weighted-random-sampler/');
  await page.fill('#in-data', 'Alice\t5\nBob\t1\nCarol\t3');
  await page.selectOption('#in-format', 'csv');
  await page.fill('#in-weight_field', '2');
  await page.fill('#in-n', '4');
  await page.check('#in-replacement');
  await page.fill('#in-seed', '7');
  await page.uncheck('#in-header');
  await page.selectOption('#in-delimiter', 'tab');

  await expect(page.locator('#in-replacement')).toBeChecked();
  await expect(page.locator('#in-header')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('Carol\t3\nCarol\t3\nAlice\t5\nAlice\t5', {
    timeout: 15000,
  });
});

test('weighted-random-sampler deep-link pre-fills JSON params and auto-runs', async ({ page }) => {
  const data = '[{"id":"a","w":5},{"id":"b","w":1},{"id":"c","w":3}]';
  const params = new URLSearchParams({
    data,
    format: 'json',
    weight_field: 'w',
    n: '2',
    replacement: 'false',
    seed: '1',
    header: 'true',
    delimiter: 'comma',
  });

  await page.goto(`/tools/weighted-random-sampler/?${params.toString()}`);
  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toHaveText('[\n  {\n    "id": "a",\n    "w": 5\n  },\n  {\n    "id": "c",\n    "w": 3\n  }\n]', { timeout: 15000 });
});
