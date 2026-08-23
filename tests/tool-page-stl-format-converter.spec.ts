import { test, expect } from './fixtures';

const TRIANGLE = `solid demo
  facet normal 0 0 1
    outer loop
      vertex 0 0 0
      vertex 10 0 0
      vertex 0 10 0
    endloop
  endfacet
endsolid demo`;

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function outputText(page: import('@playwright/test').Page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('stl-format-converter converts ASCII STL to binary data URL', async ({ page }) => {
  await page.goto('/tools/stl-format-converter/');
  await page.waitForSelector('#in-stl');
  await expect(page.locator('#in-input_format')).toHaveValue('auto');
  await expect(page.locator('#in-to')).toHaveValue('auto');

  await setField(page, '#in-stl', TRIANGLE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('data:model/stl;base64,ZGVtbw', { timeout: 20_000 });
  const text = await outputText(page);
  expect(text).toBe('data:model/stl;base64,ZGVtbwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAIA/AAAAAAAAAAAAAAAAAAAgQQAAAAAAAAAAAAAAAAAAIEEAAAAAAAA=');
});

test('stl-format-converter deep link converts binary base64 back to ASCII text', async ({ page }) => {
  const params = new URLSearchParams({
    stl: 'ZGVtbwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAIA/AAAAAAAAAAAAAAAAAAAgQQAAAAAAAAAAAAAAAAAAIEEAAAAAAAA=',
    input_format: 'base64',
    to: 'ascii',
    output: 'stl',
    normals: 'zero',
    number_format: 'decimal',
    precision: '3',
  });
  await page.goto(`/tools/stl-format-converter/?${params.toString()}`);

  await expect(page.locator('#in-input_format')).toHaveValue('base64', { timeout: 15_000 });
  await expect(page.locator('#in-to')).toHaveValue('ascii');
  await expect(page.locator('#in-normals')).toHaveValue('zero');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('solid demo', { timeout: 20_000 });
  await expect(out).toContainText('facet normal 0 0 0');
  await expect(out).toContainText('vertex 10 0 0');
  expect(await outputText(page)).toContain('endsolid demo');
});

test('stl-format-converter supports summary, enum choices, and precision boundary', async ({ page }) => {
  await page.goto('/tools/stl-format-converter/');
  await setField(page, '#in-stl', TRIANGLE);
  await page.selectOption('#in-output', 'summary');
  await page.selectOption('#in-to', 'binary');
  await page.selectOption('#in-output_encoding', 'hex');
  await page.selectOption('#in-normals', 'recompute');
  await page.selectOption('#in-number_format', 'decimal');
  await setField(page, '#in-solid_name', 'solid custom');
  await setField(page, '#in-precision', '17');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('STL format conversion', { timeout: 20_000 });
  await expect(out).toContainText('Input encoding    ASCII STL');
  await expect(out).toContainText('Output encoding   binary STL (hex)');
  await expect(out).toContainText("Facet normals     recomputed from each triangle's winding");
  await expect(out).toContainText('Note: a binary STL must not start with the word "solid"');
});

test('stl-format-converter page ships runnable CLI, labels, and preset chips', async ({ page }) => {
  await page.goto('/tools/stl-format-converter/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool stl-format-converter');
  expect(cli).toContain('solid demo');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
  await expect(page.locator('#in-input_format option[value="base64"]')).toHaveText('Binary STL bytes, base64');
  await expect(page.locator('#in-to option[value="binary"]')).toHaveText('Binary STL');
  await expect(page.locator('#in-output_encoding option[value="hex"]')).toHaveText('Raw hex bytes');
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
});
