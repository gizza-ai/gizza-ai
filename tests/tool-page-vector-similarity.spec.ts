import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

const vectors = 'apple: 1, 2, 3\nbanana: 3, 2, 1\ncherry: -3, -2, -1';

test('vector-similarity ranks cosine neighbours and shows companion metrics', async ({ page }) => {
  await page.goto('/tools/vector-similarity/');
  await setField(page, '#in-query', '3, 2, 1');
  await setField(page, '#in-vectors', vectors);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Metric: cosine similarity (higher is better)', { timeout: 15_000 });
  await expect(out).toContainText('1  banana   1.000000   14.000000');
  await expect(out).toContainText('2  apple    0.714286   10.000000   2.828427');
  await expect(out).toContainText('3  cherry  -1.000000  -14.000000');
});

test('vector-similarity honors deep-linked CSV distance parameters', async ({ page }) => {
  const params = new URLSearchParams({
    query: '3, 2, 1',
    vectors,
    metric: 'euclidean',
    top_k: '2',
    normalize: 'false',
    hamming_tolerance: '0',
    decimals: '3',
    show_all_metrics: 'false',
    output: 'csv',
  });
  await page.goto(`/tools/vector-similarity/?${params.toString()}`);

  await expect(page.locator('#in-metric')).toHaveValue('euclidean');
  await expect(page.locator('#in-show_all_metrics')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('rank,label,euclidean\n1,banana,0.000\n2,apple,2.828\n', {
    timeout: 15_000,
  });
});

test('vector-similarity covers advertised metric, output, checkbox and boundary controls', async ({ page }) => {
  await page.goto('/tools/vector-similarity/');
  await setField(page, '#in-query', '1, 2, 3');
  await setField(page, '#in-vectors', 'near: 1.05, 2, 3\nfar: 9, 9, 9');

  await page.selectOption('#in-metric', 'hamming');
  await setField(page, '#in-hamming_tolerance', '0.1');
  await page.selectOption('#in-output', 'csv');
  await page.uncheck('#in-show_all_metrics');
  await expect(page.locator('#tool-output')).toHaveText('rank,label,hamming\n1,near,0\n2,far,3\n', {
    timeout: 15_000,
  });

  await page.selectOption('#in-metric', 'dot');
  await page.selectOption('#in-output', 'json');
  await setField(page, '#in-top_k', '2000');
  await setField(page, '#in-decimals', '12');
  await setField(page, '#in-vectors', 'only: 2, 4, 6');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"metric": "dot"');
  await expect(out).toContainText('"score": 28.000000000000');
  await expect(out).toContainText('"returned": 1');
});
