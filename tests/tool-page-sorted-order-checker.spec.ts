import { test, expect } from './fixtures';

async function runWasm(
  page,
  numbers: string,
  order = 'auto',
  strict = 'false',
  separator = 'auto',
  stripThousands = 'false',
  nonNumeric = 'error',
  maxIssues = '20',
  format = 'text',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/sorted-order-checker/gizza_ai_sorted_order_checker_web.js');
    await mod.default('/tools/sorted-order-checker/gizza_ai_sorted_order_checker_web_bg.wasm');
    return mod.run(
      args.numbers,
      args.order,
      args.strict,
      args.separator,
      args.stripThousands,
      args.nonNumeric,
      args.maxIssues,
      args.format,
    );
  }, { numbers, order, strict, separator, stripThousands, nonNumeric, maxIssues, format });
}

test('sorted-order-checker wasm pinpoints the first out-of-order value', async ({ page }) => {
  await page.goto('/tools/sorted-order-checker/');
  await page.waitForSelector('#in-numbers');

  const report = await runWasm(page, '1, 4, 9, 2, 7', 'auto');
  expect(report).toContain('Not sorted ascending (equal neighbours allowed) — the order first breaks at position 4.');
  expect(report).toContain('First out-of-order element: position 4, value 2 (previous position 3, value 9)');
  expect(report).toContain('Longest sorted run: positions 1-3 (3 values)');
});

test('sorted-order-checker wasm covers enum choices, booleans, separators, json, and cap', async ({ page }) => {
  await page.goto('/tools/sorted-order-checker/');
  await page.waitForSelector('#in-numbers');

  await expect(runWasm(page, '1 2 2 5', 'ascending')).resolves.toContain('Sorted ascending');
  await expect(runWasm(page, '9 7 3', 'descending')).resolves.toContain('Sorted descending');
  await expect(runWasm(page, '1 2 2 5', 'ascending', 'true')).resolves.toContain('repeats the previous value');
  const json = JSON.parse(await runWasm(page, '1,024 2,048 4,096', 'ascending', 'false', 'space', 'true', 'error', '20', 'json'));
  expect(json).toMatchObject({ sorted: true, direction: 'ascending', first: 1024, last: 4096 });
  await expect(runWasm(page, 'n/a, 1, 2', 'ascending', 'false', 'auto', 'false', 'ignore')).resolves.toContain('Ignored non-numeric tokens: 1');
  await expect(runWasm(page, '1,2,3', 'ascending', 'false', 'comma', 'false', 'error', '1000')).resolves.toContain('Sorted ascending');
  await expect(runWasm(page, '1,2,3', 'ascending', 'false', 'comma', 'false', 'error', '1001')).rejects.toThrow(/max_issues must be between 1 and 1000/);
});

test('sorted-order-checker page renders output from controls', async ({ page }) => {
  await page.goto('/tools/sorted-order-checker/');
  await page.fill('#in-numbers', '1, 4, 9, 2, 7');
  await page.selectOption('#in-order', 'ascending');
  await page.selectOption('#in-separator', 'auto');
  await page.fill('#in-max_issues', '20');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Not sorted ascending', { timeout: 15_000 });
  await expect(out).toContainText('position 4, value 2');
});

test('sorted-order-checker deep-link prefills controls and renders strict descending output', async ({ page }) => {
  const params = new URLSearchParams({
    numbers: '100 80 80 20',
    order: 'descending',
    strict: 'true',
    separator: 'space',
    strip_thousands: 'false',
    non_numeric: 'error',
    max_issues: '20',
    format: 'text',
  });

  await page.goto(`/tools/sorted-order-checker/?${params.toString()}`);
  await expect(page.locator('#in-numbers')).toHaveValue('100 80 80 20', { timeout: 15_000 });
  await expect(page.locator('#in-order')).toHaveValue('descending');
  await expect(page.locator('#in-strict')).toBeChecked();
  await expect(page.locator('#in-separator')).toHaveValue('space');

  await expect(page.locator('#tool-output')).toContainText('Not sorted strictly descending', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('repeats the previous value');
});
