import { test, expect } from './fixtures';

const CSV = 'First Name,Last Name,Email,Mobile Phone,Company\nJohn,Doe,john@ex.com,555-1234,Acme';

const VCARD_V3 = [
  'BEGIN:VCARD',
  'VERSION:3.0',
  'N:Doe;John;;;',
  'FN:John Doe',
  'ORG:Acme',
  'EMAIL:john@ex.com',
  'TEL;TYPE=CELL:555-1234',
  'END:VCARD',
].join('\n');

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').replace(/\r\n/g, '\n').replace(/\s+$/, '');
}

test('csv-to-vcard page converts CSV contacts to exact vCard 3.0 text', async ({ page }) => {
  await page.goto('/tools/csv-to-vcard/');
  await page.fill('#in-data', CSV);
  await page.selectOption('#in-input_format', 'csv');
  await page.selectOption('#in-delimiter', 'comma');
  await page.selectOption('#in-version', '3.0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('BEGIN:VCARD', { timeout: 15000 });
  expect(await outputText(page)).toBe(VCARD_V3);
});

test('csv-to-vcard page converts JSON and vCard 4.0 fields', async ({ page }) => {
  await page.goto('/tools/csv-to-vcard/');
  await page.fill(
    '#in-data',
    '[{"Name":"Bo","Cell":"555-0000","Gender":"M","Email":"bo@example.com"}]',
  );
  await page.selectOption('#in-input_format', 'json');
  await page.selectOption('#in-version', '4.0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('VERSION:4.0', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('FN:Bo');
  expect(text).toContain('TEL;TYPE=cell:555-0000');
  expect(text).toContain('GENDER:M');
});

test('csv-to-vcard page honours semicolon delimiter', async ({ page }) => {
  await page.goto('/tools/csv-to-vcard/');
  await page.fill('#in-data', 'name;email\nAl;al@example.com\nBo;bo@example.com');
  await page.selectOption('#in-delimiter', 'semicolon');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('FN:Bo', { timeout: 15000 });
  const text = await outputText(page);
  expect((text.match(/BEGIN:VCARD/g) ?? []).length).toBe(2);
});

test('csv-to-vcard page deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/csv-to-vcard/?data=' +
      encodeURIComponent(CSV) +
      '&input_format=csv&delimiter=comma&version=3.0',
  );

  await expect(page.locator('#in-data')).toHaveValue(CSV, { timeout: 15000 });
  await expect(page.locator('#in-input_format')).toHaveValue('csv');
  await expect(page.locator('#in-delimiter')).toHaveValue('comma');
  await expect(page.locator('#in-version')).toHaveValue('3.0');
  await expect(page.locator('#tool-output')).toContainText('FN:John Doe', { timeout: 15000 });
  expect(await outputText(page)).toBe(VCARD_V3);
});
