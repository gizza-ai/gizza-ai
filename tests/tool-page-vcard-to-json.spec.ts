import { test, expect } from './fixtures';

// /tools/vcard-to-json/ parses vCard (.vcf) contact cards into a JSON array (pure wasm).
const SINGLE =
  'BEGIN:VCARD\nVERSION:3.0\nFN:John Doe\nN:Doe;John;;Mr.;\nEMAIL;TYPE=work:john@example.com\nTEL;TYPE=CELL:+15551234567\nEND:VCARD';

test('vcard-to-json parses a single contact with structured name and typed fields', async ({ page }) => {
  await page.goto('/tools/vcard-to-json/');
  await page.fill('#in-data', SINGLE);
  const out = page.locator('#tool-output');
  // structured on by default -> N split into family/given.
  await expect(out).toContainText('"family":"Doe"', { timeout: 15000 });
  await expect(out).toContainText('"given":"John"');
  await expect(out).toContainText('"fn":"John Doe"');
  await expect(out).toContainText('john@example.com');
  // TYPE=CELL lower-cased into a types array.
  await expect(out).toContainText('"cell"');
});

test('vcard-to-json wraps in an object with a count and drops structured split when toggled', async ({ page }) => {
  await page.goto('/tools/vcard-to-json/');
  await page.fill(
    '#in-data',
    'BEGIN:VCARD\nVERSION:4.0\nFN:Ada Lovelace\nN:Lovelace;Ada\nEND:VCARD\nBEGIN:VCARD\nVERSION:4.0\nFN:Alan Turing\nEND:VCARD'
  );
  await page.selectOption('#in-wrap', 'object');
  await page.uncheck('#in-structured');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"count":2', { timeout: 15000 });
  await expect(out).toContainText('"contacts"');
  // structured off -> N stays as the raw semicolon-joined string.
  await expect(out).toContainText('"name":"Lovelace;Ada"');
});

test('vcard-to-json pre-fills from query params', async ({ page }) => {
  await page.goto('/tools/vcard-to-json/?data=' + encodeURIComponent(SINGLE) + '&pretty=true');
  await expect(page.locator('#in-data')).toHaveValue(SINGLE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"fn": "John Doe"', { timeout: 15000 });
});
