import { test, expect } from './fixtures';

const sample = 'name,city,sku\n  Ada   Lovelace , New   York ,  AB 12  CD \nGrace Hopper,Berlin ,XY34';

async function runTool(
  page,
  params: {
    input?: string;
    delimiter?: string;
    trim?: string;
    internal?: string;
    whitespace?: string;
    columns?: string;
    header?: string;
    normalize_header?: string;
  } = {}
) {
  const p = {
    input: sample,
    delimiter: 'comma',
    trim: 'both',
    internal: 'collapse',
    whitespace: 'unicode',
    columns: '',
    header: 'true',
    normalize_header: 'true',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/csv-whitespace-normalizer/gizza_ai_csv_whitespace_normalizer_web.js');
    await mod.default('/tools/csv-whitespace-normalizer/gizza_ai_csv_whitespace_normalizer_web_bg.wasm');
    return mod.run(
      args.input,
      args.delimiter,
      args.trim,
      args.internal,
      args.whitespace,
      args.columns,
      args.header,
      args.normalize_header
    );
  }, p);
}

test('csv-whitespace-normalizer wasm returns exact default normalized CSV', async ({ page }) => {
  await page.goto('/tools/csv-whitespace-normalizer/');
  await page.waitForSelector('#in-input');

  const out = await runTool(page);
  expect(out).toBe('name,city,sku\nAda Lovelace,New York,AB 12 CD\nGrace Hopper,Berlin,XY34\n');
});

test('csv-whitespace-normalizer deep-link prefills controls and page output', async ({ page }) => {
  await page.goto('/tools/csv-whitespace-normalizer/?delimiter=auto&trim=both&internal=remove&whitespace=unicode&columns=sku&header=true&normalize_header=false');
  await page.waitForSelector('#in-input');
  await expect(page.locator('#in-delimiter')).toHaveValue('auto');
  await expect(page.locator('#in-trim')).toHaveValue('both');
  await expect(page.locator('#in-internal')).toHaveValue('remove');
  await expect(page.locator('#in-whitespace')).toHaveValue('unicode');
  await expect(page.locator('#in-columns')).toHaveValue('sku');
  await expect(page.locator('#in-header')).toBeChecked();
  await expect(page.locator('#in-normalize_header')).not.toBeChecked();

  await page.fill('#in-input', sample);
  await expect(page.locator('#tool-output')).toContainText('AB12CD', { timeout: 10_000 });
  await expect(page.locator('#tool-output')).toContainText('Ada   Lovelace');
});

test('csv-whitespace-normalizer covers advertised delimiter, trim, internal and whitespace modes', async ({ page }) => {
  await page.goto('/tools/csv-whitespace-normalizer/');
  await page.waitForSelector('#in-input');

  const keep = await runTool(page, { input: 'a,b\n  x   y  ,  z \n', internal: 'keep' });
  expect(keep).toBe('a,b\nx   y,z\n');

  const leading = await runTool(page, { input: 'a\n  x  \n', trim: 'leading', internal: 'keep' });
  expect(leading).toBe('a\nx  \n');

  const tsv = await runTool(page, { input: 'a\tb\n  x  \t  y  \n', delimiter: 'auto' });
  expect(tsv).toBe('a\tb\nx\ty\n');

  const unicode = await runTool(page, { input: 'name\n\u00a0Ada\u3000\u3000Lovelace\u202f\n' });
  expect(unicode).toBe('name\nAda Lovelace\n');

  const ascii = await runTool(page, { input: 'name\n \u00a0Ada  Lovelace \n', whitespace: 'ascii' });
  expect(ascii).toBe('name\n\u00a0Ada Lovelace\n');
});

test('csv-whitespace-normalizer ships presets and reports validation errors', async ({ page }) => {
  await page.goto('/tools/csv-whitespace-normalizer/');
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
  await page.click('.tool-example-chip:has-text("Squeeze one SKU column shut")');
  await expect(page.locator('#in-internal')).toHaveValue('remove');
  await expect(page.locator('#in-columns')).toHaveValue('sku');

  await expect(runTool(page, { columns: '9' })).rejects.toThrow(/out of range/);
});
