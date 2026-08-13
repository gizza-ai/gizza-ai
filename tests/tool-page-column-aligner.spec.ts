import { test, expect } from './fixtures';

const INPUT = 'name age city\nalice 30 Berlin\nbo 7 SF';
const ALIGNED = 'name   age  city\nalice  30   Berlin\nbo     7    SF';

async function runWasm(
  page: any,
  input: string,
  delimiter = 'whitespace',
  align = 'left',
  columnAlign = '',
  gap = '2',
  separator = '',
  trim = 'true',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/column-aligner/gizza_ai_column_aligner_web.js');
    await mod.default('/tools/column-aligner/gizza_ai_column_aligner_web_bg.wasm');
    return mod.run(
      args.input,
      args.delimiter,
      args.align,
      args.columnAlign,
      args.gap,
      args.separator,
      args.trim,
    );
  }, { input, delimiter, align, columnAlign, gap, separator, trim });
}

test('column-aligner wasm aligns whitespace columns exactly', async ({ page }) => {
  await page.goto('/tools/column-aligner/');
  await expect(runWasm(page, INPUT)).resolves.toBe(ALIGNED);
  await expect(runWasm(page, 'a,  bb\nccc,d', 'comma')).resolves.toBe('a    bb\nccc  d');
  await expect(runWasm(page, 'a 1\nbbb 22', 'whitespace', 'left', '', '1', '|'))
    .resolves.toBe('a   | 1\nbbb | 22');
});

test('column-aligner page computes exact output from the form', async ({ page }) => {
  await page.goto('/tools/column-aligner/');
  await page.fill('#in-input', INPUT);
  await expect(page.locator('#tool-output')).toHaveText(ALIGNED, { timeout: 15_000 });
});

test('column-aligner deep link covers separator and non-default checkbox state', async ({ page }) => {
  const params = new URLSearchParams({
    input: 'a, bb\nccc,d',
    delimiter: 'comma',
    align: 'left',
    column_align: '',
    gap: '1',
    separator: '|',
    trim: 'false',
  });
  await page.goto(`/tools/column-aligner/?${params.toString()}`);

  await expect(page.locator('#in-delimiter')).toHaveValue('comma', { timeout: 15_000 });
  await expect(page.locator('#in-gap')).toHaveValue('1');
  await expect(page.locator('#in-separator')).toHaveValue('|');
  await expect(page.locator('#in-trim')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('a   |  bb\nccc | d', { timeout: 15_000 });
});

test('column-aligner covers enum values, gap cap, Unicode width, and CLI example', async ({ page }) => {
  await page.goto('/tools/column-aligner/');

  await expect(runWasm(page, 'a 1\nbbb 22', 'whitespace', 'right', '', '1'))
    .resolves.toBe('  a  1\nbbb 22');
  await expect(runWasm(page, 'a\nbbbb', 'whitespace', 'center'))
    .resolves.toBe(' a\nbbbb');
  await expect(runWasm(page, 'widget 5\nbolt 1200', 'whitespace', 'auto'))
    .resolves.toBe('widget     5\nbolt    1200');
  await expect(runWasm(page, 'a 1\nbbb 2222', 'whitespace', 'left', 'lr'))
    .resolves.toBe('a       1\nbbb  2222');
  await expect(runWasm(page, '東京 1\nab 2')).resolves.toBe('東京  1\nab    2');
  await expect(runWasm(page, 'a b', 'whitespace', 'left', '', '16')).resolves.toBe('a                b');
  await expect(runWasm(page, 'a b', 'whitespace', 'left', '', '17')).rejects.toThrow(/between 0 and 16/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool column-aligner');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
