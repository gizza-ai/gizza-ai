import { test, expect } from './fixtures';

const SAMPLE = 'INV-1001\nINV-1002\nINV-1004\nINV-1004\nINV-1007';
const SAMPLE_REPORT = 'Range: INV-1000 to INV-1008 (step 1)\nPresent: 4 of 9 expected\nMissing: 5 (INV-1000, INV-1003, INV-1005 to INV-1006, INV-1008)\nDuplicates: 1 (INV-1004 x2)';

async function runWasm(
  page: any,
  data = SAMPLE,
  id_format = 'auto',
  separator = 'newline',
  step = '1',
  start = 'INV-1000',
  end = 'INV-1008',
  order = 'sorted',
  duplicates = true,
  output = 'report',
  limit = '1000',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/gap-finder/gizza_ai_gap_finder_web.js');
    await mod.default('/tools/gap-finder/gizza_ai_gap_finder_web_bg.wasm');
    return mod.run(
      args.data,
      args.id_format,
      args.separator,
      args.step,
      args.start,
      args.end,
      args.order,
      args.duplicates ? 'true' : 'false',
      args.output,
      args.limit,
    );
  }, { data, id_format, separator, step, start, end, order, duplicates, output, limit });
}

test('gap-finder wasm reports prefixed gaps and duplicates exactly', async ({ page }) => {
  await page.goto('/tools/gap-finder/');
  await page.waitForSelector('#in-data');

  expect(await runWasm(page)).toBe(SAMPLE_REPORT);
  expect(await runWasm(page, '10\n14', 'number', 'newline', '1', '', '', 'sorted', true, 'missing')).toBe('11\n12\n13');
  expect(await runWasm(page, '1001\n1002\n1006', 'number', 'newline', '1', '', '', 'sorted', true, 'table'))
    .toBe('gap_start\tgap_end\tcount\n1003\t1005\t3');
});

test('gap-finder page computes exact report from form controls', async ({ page }) => {
  await page.goto('/tools/gap-finder/');
  await page.fill('#in-data', SAMPLE);
  await page.selectOption('#in-id_format', 'auto');
  await page.selectOption('#in-separator', 'newline');
  await page.fill('#in-step', '1');
  await page.fill('#in-start', 'INV-1000');
  await page.fill('#in-end', 'INV-1008');
  await page.selectOption('#in-order', 'sorted');
  await page.check('#in-duplicates');
  await page.selectOption('#in-output', 'report');
  await page.fill('#in-limit', '1000');

  await expect(page.locator('#tool-output')).toHaveText(SAMPLE_REPORT, { timeout: 15_000 });
});

test('gap-finder deep link covers order mode and non-default checkbox', async ({ page }) => {
  const params = new URLSearchParams({
    data: '1\n3\n2\n4',
    id_format: 'number',
    separator: 'newline',
    step: '1',
    start: '',
    end: '',
    order: 'input',
    duplicates: 'false',
    output: 'report',
    limit: '1000',
  });
  await page.goto(`/tools/gap-finder/?${params.toString()}`);

  await expect(page.locator('#in-order')).toHaveValue('input', { timeout: 15_000 });
  await expect(page.locator('#in-duplicates')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('Range: 1 to 4 (step 1)\nPresent: 4 of 4 expected\nMissing: none\nOut of order: 1 (2 after 3)', { timeout: 15_000 });
});

test('gap-finder covers enum choices, errors, limit boundary, and CLI example', async ({ page }) => {
  await page.goto('/tools/gap-finder/');
  await page.waitForSelector('#in-data');

  expect(await runWasm(page, '2\n4\n5\n10', 'number', 'newline', '2', '', '', 'sorted', true, 'report'))
    .toBe('Range: 2 to 10 (step 2)\nPresent: 3 of 5 expected\nMissing: 2 (6-8)\nDuplicates: none\nOff-step: 1 (5)');

  const json = await runWasm(page, '1001\n1001\n1004', 'number', 'newline', '1', '', '', 'sorted', true, 'json');
  expect(json).toContain('"missing_count": 2');
  expect(json).toContain('{ "value": "1001", "count": 2 }');

  expect(await runWasm(page, '1\n100', 'number', 'newline', '1', '', '', 'sorted', true, 'missing', '3'))
    .toBe('2\n3\n4\n# 95 more missing values not shown — raise the limit to list them');
  await expect(runWasm(page, 'INV-1\nORD-2')).rejects.toThrow(/must share the same prefix and suffix/);
  await expect(runWasm(page, '1\n2', 'number', 'newline', '1', '', '', 'sorted', true, 'report', '0'))
    .rejects.toThrow(/limit must be between/);

  const atCap = Array.from({ length: 20000 }, (_, i) => String(i + 1)).join('\n');
  expect(await runWasm(page, atCap, 'number', 'newline', '1', '', '', 'sorted', true, 'report', '1000'))
    .toBe('Range: 1 to 20000 (step 1)\nPresent: 20000 of 20000 expected\nMissing: none\nDuplicates: none');
  await expect(runWasm(page, `${atCap}\n20001`, 'number', 'newline')).rejects.toThrow(/exceeds the maximum/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool gap-finder');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
