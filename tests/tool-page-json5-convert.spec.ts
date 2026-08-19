import { test, expect } from './fixtures';

const output = (page) =>
  page.locator('#tool-output').evaluate((el) => el.textContent?.trim() ?? '');

test('json5-convert page strips JSONC comments and trailing commas', async ({ page }) => {
  await page.goto('/tools/json5-convert/');
  await page.fill(
    '#in-text',
    "{\n  // dev server\n  port: 8080,\n  hosts: ['a', 'b',],\n}",
  );

  await expect(page.locator('#tool-output')).toContainText('"port"', { timeout: 15000 });
  expect(await output(page)).toBe(
    '{\n' +
      '  "port": 8080,\n' +
      '  "hosts": [\n' +
      '    "a",\n' +
      '    "b"\n' +
      '  ]\n' +
      '}',
  );
});

test('json5-convert deep-link minifies and stringifies non-finite values', async ({ page }) => {
  const text = encodeURIComponent('{ mask: 0xff, ratio: .5, missing: NaN, cap: Infinity }');
  await page.goto(
    `/tools/json5-convert/?text=${text}&direction=to-json&indent=minify&sort_keys=false&nonfinite=string&quote_style=single&unquote_keys=true&trailing_commas=false`,
  );

  await expect(page.locator('#in-direction')).toHaveValue('to-json');
  await expect(page.locator('#in-indent')).toHaveValue('minify');
  await expect(page.locator('#in-nonfinite')).toHaveValue('string');
  await expect(page.locator('#in-unquote_keys')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('"Infinity"', { timeout: 15000 });
  expect(await output(page)).toBe('{"mask":255,"ratio":0.5,"missing":"NaN","cap":"Infinity"}');
});

test('json5-convert page writes JSON5 with double quotes and trailing commas', async ({ page }) => {
  await page.goto('/tools/json5-convert/');
  await page.fill('#in-text', '{"name":"ada","tags":["x","y"]}');
  await page.selectOption('#in-direction', 'to-json5');
  await page.selectOption('#in-quote_style', 'double');
  await page.uncheck('#in-unquote_keys');
  await page.check('#in-trailing_commas');

  await expect(page.locator('#tool-output')).toContainText('"name"', { timeout: 15000 });
  await expect(page.locator('#in-trailing_commas')).toBeChecked();
  await expect(page.locator('#in-unquote_keys')).not.toBeChecked();
  expect(await output(page)).toBe(
    '{\n' +
      '  "name": "ada",\n' +
      '  "tags": [\n' +
      '    "x",\n' +
      '    "y",\n' +
      '  ],\n' +
      '}',
  );
});

test('json5-convert auto direction sorts keys for strict input', async ({ page }) => {
  await page.goto('/tools/json5-convert/');
  await page.fill('#in-text', '{"z":1,"a":{"d":4,"b":2}}');
  await page.selectOption('#in-direction', 'auto');
  await page.check('#in-sort_keys');

  await expect(page.locator('#in-sort_keys')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('a:', { timeout: 15000 });
  expect(await output(page)).toBe(
    '{\n' +
      '  a: {\n' +
      '    b: 2,\n' +
      '    d: 4\n' +
      '  },\n' +
      '  z: 1\n' +
      '}',
  );
});

test('json5-convert page reports non-finite strict JSON errors', async ({ page }) => {
  await page.goto('/tools/json5-convert/');
  await page.fill('#in-text', '{ value: NaN }');
  await page.selectOption('#in-nonfinite', 'error');

  await expect(page.locator('#tool-output')).toContainText(
    "'NaN' has no strict-JSON equivalent",
    { timeout: 15000 },
  );
});
