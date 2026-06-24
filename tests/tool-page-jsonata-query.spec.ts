import { test, expect } from './fixtures';

// /tools/jsonata-query/ evaluates a JSONata expression over JSON in-browser
// (pure-Rust jsonata-rs). Fields: query, json (multiline), pretty (checkbox).
test('jsonata-query page aggregates and navigates JSON', async ({ page }) => {
  await page.goto('/tools/jsonata-query/');
  await page.fill('#in-query', '$sum(items.price)');
  await page.fill('#in-json', '{"items":[{"price":2},{"price":3},{"price":5}]}');
  await expect(page.locator('#tool-output')).toContainText('10', { timeout: 15000 });

  // Path navigation into nested objects.
  await page.fill('#in-query', 'Account.Order.Product.Price');
  await page.fill('#in-json', '{"Account":{"Order":{"Product":{"Price":34.5}}}}');
  await expect(page.locator('#tool-output')).toContainText('34.5', { timeout: 15000 });
});

test('jsonata-query page deep-link filters and projects', async ({ page }) => {
  const qs =
    '?query=' + encodeURIComponent('items[price > 10].name') +
    '&json=' + encodeURIComponent('{"items":[{"name":"a","price":5},{"name":"b","price":20},{"name":"c","price":30}]}');
  await page.goto('/tools/jsonata-query/' + qs);
  await expect(page.locator('#in-query')).toHaveValue('items[price > 10].name', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('"b"', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('"c"');
  await expect(page.locator('#tool-output')).not.toContainText('"a"');
});

test('jsonata-query page pretty-print checkbox indents output', async ({ page }) => {
  await page.goto('/tools/jsonata-query/');
  await page.fill('#in-query', '$');
  await page.fill('#in-json', '{"a":1}');
  const out = page.locator('#tool-output');
  // Compact by default — a single line, no indentation.
  await expect(out).toContainText('"a"', { timeout: 15000 });
  expect((await out.textContent())!).not.toContain('\n');
  // Tick pretty-print → the serialized JSON gains newlines + indentation.
  await page.check('#in-pretty');
  await expect(out).toContainText('"a": 1', { timeout: 15000 }); // space after colon = indented
  expect((await out.textContent())!).toContain('\n  "a": 1');
});

test('jsonata-query page reports no-match and invalid expressions', async ({ page }) => {
  await page.goto('/tools/jsonata-query/');
  // A path that matches nothing normalizes to JSON null.
  await page.fill('#in-query', 'missing.field');
  await page.fill('#in-json', '{"a":1}');
  await expect(page.locator('#tool-output')).toContainText('null', { timeout: 15000 });

  // An unparseable expression surfaces an error on the output.
  await page.fill('#in-query', 'a +');
  await page.fill('#in-json', '1');
  await expect(page.locator('#tool-output')).toContainText('invalid JSONata expression', { timeout: 15000 });
});
