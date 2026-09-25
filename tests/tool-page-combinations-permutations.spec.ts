import { test, expect } from './fixtures';

async function setText(page: import('@playwright/test').Page, selector: string, value: string) {
  await page.locator(selector).evaluate((el, v) => {
    const input = el as HTMLInputElement | HTMLTextAreaElement;
    input.value = v;
    input.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('combinations-permutations summarizes lottery odds exactly', async ({ page }) => {
  await page.goto('/tools/combinations-permutations/');
  await setText(page, '#in-n', '49');
  await setText(page, '#in-r', '6');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('C(49, 6) = 13,983,816', { timeout: 15_000 });
  await expect(out).toContainText('Formula: C(n, r) = n! / (r! * (n - r)!) = 49! / (6! * 43!)');
  await expect(out).toContainText('Odds of one specific result: 1 in 13,983,816');
});

test('combinations-permutations lists item combinations', async ({ page }) => {
  await page.goto('/tools/combinations-permutations/');
  await setText(page, '#in-items', 'A, B, C, D');
  await setText(page, '#in-r', '2');
  await page.selectOption('#in-output_format', 'lines');

  await expect(page.locator('#tool-output')).toHaveText('A, B\nA, C\nA, D\nB, C\nB, D\nC, D', { timeout: 15_000 });
});

test('combinations-permutations covers permutations, repetition, and count output', async ({ page }) => {
  await page.goto('/tools/combinations-permutations/');
  await setText(page, '#in-n', '10');
  await setText(page, '#in-r', '3');
  await page.selectOption('#in-mode', 'permutations');
  await page.check('#in-repetition');
  await page.selectOption('#in-output_format', 'count');

  await expect(page.locator('#tool-output')).toHaveText('1000', { timeout: 15_000 });
});

test('combinations-permutations emits CSV rows with alternate item separator', async ({ page }) => {
  await page.goto('/tools/combinations-permutations/');
  await setText(page, '#in-items', 'red|green|blue');
  await setText(page, '#in-r', '2');
  await page.selectOption('#in-item_separator', 'pipe');
  await page.selectOption('#in-output_format', 'csv');

  await expect(page.locator('#tool-output')).toHaveText('red,green\nred,blue\ngreen,blue', { timeout: 15_000 });
});

test('combinations-permutations deep-links circular seating with dash joiner', async ({ page }) => {
  const qs = new URLSearchParams({
    items: 'Ada, Ben, Cy, Dee',
    r: '4',
    mode: 'circular_permutations',
    output_format: 'lines',
    join_separator: 'dash',
  });
  await page.goto(`/tools/combinations-permutations/?${qs.toString()}`);

  await expect(page.locator('#in-mode')).toHaveValue('circular_permutations');
  await expect(page.locator('#in-join_separator')).toHaveValue('dash');
  await expect(page.locator('#tool-output')).toHaveText(
    'Ada-Ben-Cy-Dee\nAda-Ben-Dee-Cy\nAda-Cy-Ben-Dee\nAda-Cy-Dee-Ben\nAda-Dee-Ben-Cy\nAda-Dee-Cy-Ben',
    { timeout: 15_000 },
  );
});

test('combinations-permutations rejects enumeration above cap with exact total', async ({ page }) => {
  await page.goto('/tools/combinations-permutations/');
  await setText(page, '#in-n', '49');
  await setText(page, '#in-r', '6');
  await page.selectOption('#in-output_format', 'lines');

  await expect(page.locator('#tool-output')).toContainText('13983816 results, above max_results (10000)', { timeout: 15_000 });
});
