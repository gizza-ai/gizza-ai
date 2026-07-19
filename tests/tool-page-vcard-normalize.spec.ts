import { test, expect } from './fixtures';

const sample = [
  'BEGIN:VCARD',
  'VERSION:3.0',
  'FN: jane   DOE',
  'N:DOE;jane;;;',
  'TEL;TYPE=CELL:(415) 555-2671',
  'EMAIL: Jane.DOE@Example.COM ',
  'X-CUSTOM:Keep Me',
  'END:VCARD',
].join('\n');

test('vcard-normalize cleans email, phone, and names', async ({ page }) => {
  await page.goto('/tools/vcard-normalize/');
  await page.fill('#in-data', sample);
  await page.fill('#in-default_country', 'US');
  await page.selectOption('#in-name_case', 'title');
  await expect(page.locator('#tool-output')).toContainText('EMAIL:jane.doe@example.com', {
    timeout: 15000,
  });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toContain('FN:Jane Doe');
  expect(out).toContain('N:Doe;Jane;;;');
  expect(out).toContain('TEL;TYPE=CELL:+14155552671');
  expect(out).toContain('X-CUSTOM:Keep Me');
});

test('vcard-normalize can preserve email case and name case', async ({ page }) => {
  await page.goto('/tools/vcard-normalize/');
  await page.fill('#in-data', 'BEGIN:VCARD\nVERSION:4.0\nFN:McDonald   Family\nEMAIL: INFO@Example.ORG \nEND:VCARD');
  await page.uncheck('#in-lowercase_email');
  await expect(page.locator('#tool-output')).toContainText('EMAIL:INFO@Example.ORG', {
    timeout: 15000,
  });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toContain('FN:McDonald Family');
});

test('vcard-normalize deep-links and auto-runs', async ({ page }) => {
  const data = 'BEGIN:VCARD\nVERSION:3.0\nFN: london   OFFICE\nTEL:+44 20 7183 8750\nEMAIL:Office@Example.co.uk\nEND:VCARD';
  await page.goto(
    '/tools/vcard-normalize/?' +
      new URLSearchParams({ data, name_case: 'upper', lowercase_email: 'true' }).toString()
  );
  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('FN:LONDON OFFICE', { timeout: 15000 });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toContain('TEL:+442071838750');
  expect(out).toContain('EMAIL:office@example.co.uk');
});
