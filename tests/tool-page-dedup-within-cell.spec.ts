import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  data: string,
  columns = '',
  item_separator = 'comma',
  output_separator = '',
  ignore_case = 'false',
  trim_items = 'true',
  drop_empty = 'true',
  sort_items = 'none',
  delimiter = 'comma',
  has_header = 'true',
  output = 'csv',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/dedup-within-cell/gizza_ai_dedup_within_cell_web.js');
    await mod.default('/tools/dedup-within-cell/gizza_ai_dedup_within_cell_web_bg.wasm');
    return mod.run(
      args.data,
      args.columns,
      args.item_separator,
      args.output_separator,
      args.ignore_case,
      args.trim_items,
      args.drop_empty,
      args.sort_items,
      args.delimiter,
      args.has_header,
      args.output,
    );
  }, { data, columns, item_separator, output_separator, ignore_case, trim_items, drop_empty, sort_items, delimiter, has_header, output });
}

test('dedup-within-cell page cleans duplicate items inside selected cells', async ({ page }) => {
  await page.goto('/tools/dedup-within-cell/');
  await page.fill('#in-data', 'id,tags\n1,"a, b, a, c"\n2,"red, blue, red"');
  await page.fill('#in-columns', 'tags');
  await page.selectOption('#in-item_separator', 'comma');
  await page.selectOption('#in-sort_items', 'none');
  await page.selectOption('#in-output', 'csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('id,tags', { timeout: 15_000 });
  await expect(out).toContainText('1,"a, b, c"');
  await expect(out).toContainText('2,"red, blue"');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool dedup-within-cell');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('dedup-within-cell deep-link prefills and produces a stats report', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'id,tags\n1,"a, b, a"\n2,"c, c, c"',
    columns: 'tags',
    item_separator: 'comma',
    output_separator: '',
    ignore_case: 'false',
    trim_items: 'true',
    drop_empty: 'true',
    sort_items: 'none',
    delimiter: 'comma',
    has_header: 'true',
    output: 'stats',
  });
  await page.goto(`/tools/dedup-within-cell/?${params.toString()}`);
  await expect(page.locator('#in-columns')).toHaveValue('tags', { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('stats');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('data rows: 2', { timeout: 15_000 });
  await expect(out).toContainText('cells scanned: 2');
  await expect(out).toContainText('cells changed: 2');
  await expect(out).toContainText('duplicate items removed: 3');
});

test('dedup-within-cell wasm covers enums, checkboxes, stats and TSV', async ({ page }) => {
  await page.goto('/tools/dedup-within-cell/');
  await page.waitForSelector('#in-data');

  const asc = await runWasm(page, 'tags\n"banana, Apple, apple, Cherry"\n', '', 'comma', ', ', 'true', 'true', 'true', 'asc');
  expect(asc).toBe('tags\n"Apple, banana, Cherry"\n');

  const keepEmpty = await runWasm(page, 'tags\n"a,,b,,a"\n', '', 'comma', '', 'false', 'true', 'false');
  expect(keepEmpty).toBe('tags\n"a,,b"\n');

  const stats = await runWasm(page, 'id,tags\n1,"a, b, a"\n2,"c, c, c"\n', 'tags', 'comma', '', 'false', 'true', 'true', 'none', 'comma', 'true', 'stats');
  expect(stats).toBe('data rows: 2\ncells scanned: 2\ncells changed: 2\nduplicate items removed: 3\n');

  const tsv = await runWasm(page, 'id\ttags\n1\ta, b, a\n', '', 'comma', '', 'false', 'true', 'true', 'none', 'tab');
  expect(tsv).toBe('id\ttags\n1\ta, b\n');

  await expect(runWasm(page, 'id,tags\n1,a\n', 'missing')).rejects.toThrow(/column 'missing' not found/);
});
