import { test, expect } from './fixtures';

const DATA = 'id,label\n1,a\n2,b\n3,a\n4,b\n5,a\n6,b\n7,a\n8,b\n9,a\n10,b';

async function runWasm(
  page: any,
  data = DATA,
  test_size = 0.2,
  validation_size = 0,
  stratify_column = '',
  stratify_bins = 0,
  group_column = '',
  shuffle = 'true',
  seed = 42,
  header = 'true',
  delimiter = 'comma',
  output = 'sections',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/train-test-split/gizza_ai_train_test_split_web.js');
    await mod.default('/tools/train-test-split/gizza_ai_train_test_split_web_bg.wasm');
    return mod.run(
      args.data,
      args.test_size,
      args.validation_size,
      args.stratify_column,
      args.stratify_bins,
      args.group_column,
      args.shuffle,
      args.seed,
      args.header,
      args.delimiter,
      args.output,
    );
  }, { data, test_size, validation_size, stratify_column, stratify_bins, group_column, shuffle, seed, header, delimiter, output });
}

test('train-test-split page produces real labelled splits', async ({ page }) => {
  await page.goto('/tools/train-test-split/');
  await page.fill('#in-data', DATA);
  await page.fill('#in-test_size', '0.2');
  await page.fill('#in-validation_size', '0');
  await page.selectOption('#in-output', 'sections');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('# train (8 rows)', { timeout: 20_000 });
  await expect(output).toContainText('# test (2 rows)');
  await expect(output).toContainText('id,label');
});

test('train-test-split deep link supports sequential no-shuffle split', async ({ page }) => {
  const params = new URLSearchParams({
    data: DATA,
    test_size: '0.2',
    validation_size: '0.1',
    stratify_column: '',
    stratify_bins: '0',
    group_column: '',
    shuffle: 'false',
    seed: '42',
    header: 'true',
    delimiter: 'comma',
    output: 'test',
  });
  await page.goto(`/tools/train-test-split/?${params.toString()}`);

  await expect(page.locator('#in-shuffle')).not.toBeChecked({ timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('test');
  await expect(page.locator('#tool-output')).toHaveText('id,label\n9,a\n10,b\n', { timeout: 20_000 });
});

test('train-test-split covers wasm enums, stratification, grouping, boundaries, and errors', async ({ page }) => {
  await page.goto('/tools/train-test-split/');
  await page.waitForSelector('#in-data');

  expect(await runWasm(page, DATA, 0.4, 0, 'label', 0, '', 'true', 3, 'true', 'comma', 'summary'))
    .toContain('split,class,rows,percent');

  const grouped = await runWasm(page, 'patient,visit\np1,1\np1,2\np2,1\np2,2\np3,1\np3,2\np4,1\np4,2', 0.25, 0, '', 0, 'patient', 'true', 9, 'true', 'comma', 'test');
  const groupedIds = grouped.trim().split('\n').slice(1).map((line: string) => line.split(',')[0]);
  expect(groupedIds.length).toBe(2);
  expect(groupedIds[0]).toBe(groupedIds[1]);

  expect(await runWasm(page, 'id\tlabel\n1\ta\n2\tb\n3\ta\n4\tb', 0.5, 0, '', 0, '', 'false', 42, 'true', 'tab', 'test'))
    .toBe('id\tlabel\n3\ta\n4\tb\n');

  const json = await runWasm(page, DATA, 1, 0, '', 0, '', 'true', 42, 'true', 'comma', 'json');
  expect(json).toContain('"test": 1');
  expect(json).toContain('"total": 10');

  await expect(runWasm(page, DATA, 0.2, 0, 'label', 0, '', 'false', 42, 'true', 'comma', 'test'))
    .rejects.toThrow(/shuffle/);
});

test('train-test-split generated CLI example is generic and runnable-looking', async ({ page }) => {
  await page.goto('/tools/train-test-split/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool train-test-split');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
