import { test, expect } from './fixtures';

const messyCard = [
  'BEGIN:VCARD',
  'VERSION:3.0',
  'FN:Ada Lovelace',
  'N:Lovelace;Ada',
  'EMAIL:ada@@example.com',
  'TEL;WORK:+44 1632 960 961',
  'BDAY:1815-13-40',
  'URL:example.com',
  'END:VCARD',
].join('\n');

test('vcard-validator reports exact issues for a messy card', async ({ page }) => {
  await page.goto('/tools/vcard-validator/');
  await page.fill('#in-data', messyCard);
  await page.selectOption('#in-version', 'auto');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('INVALID — 1 card, 5 errors, 2 warnings', {
    timeout: 15000,
  });
  await expect(out).toContainText('line      4  error    invalid-n');
  await expect(out).toContainText('line      5  error    invalid-email');
  await expect(out).toContainText('line      6  error    bare-parameter');
  await expect(out).toContainText('line      6  error    invalid-tel');
  await expect(out).toContainText('line      7  error    invalid-date');
  await expect(out).toContainText('line      8  warning  invalid-uri');
});

test('vcard-validator deep-links country and JSON output', async ({ page }) => {
  const data = [
    'BEGIN:VCARD',
    'VERSION:4.0',
    'FN:Grace Hopper',
    'TEL:(650) 253-0000',
    'END:VCARD',
  ].join('\n');
  await page.goto(
    '/tools/vcard-validator/?' +
      new URLSearchParams({
        data,
        version: '4.0',
        default_country: 'US',
        check_email: 'false',
        output: 'json',
      }).toString()
  );

  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  await expect(page.locator('#in-version')).toHaveValue('4.0');
  await expect(page.locator('#in-default_country')).toHaveValue('US');
  await expect(page.locator('#in-check_email')).not.toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"error_count": 0', { timeout: 15000 });
  await expect(out).toContainText('"rule": "tel-not-uri"');
  await expect(out).toContainText('"severity": "warning"');
});
