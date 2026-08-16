import { test, expect } from './fixtures';

const tool = '/tools/csv-header-sanitizer/';
const messy = 'First Name, Total ($) ,2024 Revenue,,Notes,Notes\nAda,10,120,x,first,second';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return text ?? '';
}

async function runWasm(
  page,
  data: string,
  delimiter = ',',
  style = 'snake',
  ascii = 'true',
  leadingDigit = 'underscore',
  maxLength = '0',
  blankName = 'column',
  dedupe = 'suffix',
  output = 'csv',
) {
  return await page.evaluate(
    async ({ data, delimiter, style, ascii, leadingDigit, maxLength, blankName, dedupe, output }) => {
      const mod = await import('/tools/csv-header-sanitizer/gizza_ai_csv_header_sanitizer_web.js');
      await mod.default('/tools/csv-header-sanitizer/gizza_ai_csv_header_sanitizer_web_bg.wasm');
      return mod.run(data, delimiter, style, ascii, leadingDigit, maxLength, blankName, dedupe, output);
    },
    { data, delimiter, style, ascii, leadingDigit, maxLength, blankName, dedupe, output },
  );
}

test('csv-header-sanitizer page cleans a messy header row with exact output', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', messy);
  await page.selectOption('#in-style', 'snake');
  await page.check('#in-ascii');
  await page.selectOption('#in-leading_digit', 'underscore');
  await page.fill('#in-max_length', '0');
  await page.fill('#in-blank_name', 'column');
  await page.selectOption('#in-dedupe', 'suffix');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toContainText('first_name,total,_2024_revenue,column_4,notes,notes_2', { timeout: 15000 });
  expect((await outputText(page)).trim()).toBe(
    'first_name,total,_2024_revenue,column_4,notes,notes_2\nAda,10,120,x,first,second',
  );
});

test('csv-header-sanitizer deep link prefills mapping output and non-default checkbox state', async ({ page }) => {
  await page.goto(
    tool +
      '?data=' +
      encodeURIComponent('Año,Größe\n1,2') +
      '&delimiter=%2C&style=snake&ascii=false&leading_digit=underscore&max_length=0&blank_name=column&dedupe=suffix&output=mapping',
  );
  await expect(page.locator('#in-data')).toHaveValue('Año,Größe\n1,2', { timeout: 15000 });
  await expect(page.locator('#in-ascii')).not.toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('mapping');

  await expect(page.locator('#tool-output')).toContainText('original,sanitized');
  await expect(page.locator('#tool-output')).toContainText('Año,año');
  await expect(page.locator('#tool-output')).toContainText('Größe,größe');
});

test('csv-header-sanitizer wasm covers advertised styles, delimiter forms, dedupe and length cap', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-data');

  expect(await runWasm(page, 'First Name,HTTPStatusCode', ',', 'camel', 'true', 'underscore', '0', 'column', 'suffix', 'header')).toBe('firstName,httpStatusCode\n');
  expect(await runWasm(page, 'First Name,HTTPStatusCode', ',', 'pascal', 'true', 'underscore', '0', 'column', 'suffix', 'header')).toBe('FirstName,HttpStatusCode\n');
  expect(await runWasm(page, 'First Name,HTTPStatusCode', ',', 'kebab', 'true', 'underscore', '0', 'column', 'suffix', 'header')).toBe('first-name,http-status-code\n');
  expect(await runWasm(page, 'First Name,HTTPStatusCode', ',', 'screaming_snake', 'true', 'underscore', '0', 'column', 'suffix', 'header')).toBe('FIRST_NAME,HTTP_STATUS_CODE\n');
  expect(await runWasm(page, 'First Name,HTTPStatusCode', ',', 'preserve', 'true', 'underscore', '0', 'column', 'suffix', 'header')).toBe('First_Name,HTTPStatusCode\n');

  expect(await runWasm(page, 'First Name\tTotal ($)\nAda\t10', 'auto')).toBe('first_name\ttotal\nAda\t10\n');
  expect(await runWasm(page, '2024 Revenue', ',', 'snake', 'true', 'col', '0', 'column', 'suffix', 'header')).toBe('col_2024_revenue\n');
  expect(await runWasm(page, 'Total,TOTAL', ',', 'snake', 'true', 'underscore', '0', 'column', 'allow', 'header')).toBe('total,total\n');
  expect(await runWasm(page, 'Customer Lifetime Value,Customer Lifetime Value', ',', 'snake', 'true', 'underscore', '12', 'column', 'suffix', 'header')).toBe('customer_lif,customer_l_2\n');

  await expect(runWasm(page, 'a,b', '::')).rejects.toThrow(/delimiter must be/);
});

test('csv-header-sanitizer page ships workflow example presets', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
  await page.click('.tool-example-chip:has-text("camelCase")');
  await expect(page.locator('#in-style')).toHaveValue('camel');
  await expect(page.locator('#in-output')).toHaveValue('header');
});
