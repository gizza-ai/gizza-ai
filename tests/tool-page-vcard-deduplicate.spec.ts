import { test, expect } from './fixtures';

const TWO_JOHNS = [
  'BEGIN:VCARD',
  'VERSION:3.0',
  'FN:John Doe',
  'EMAIL:john@work.com',
  'TEL:+1-555-111-2222',
  'END:VCARD',
  'BEGIN:VCARD',
  'VERSION:3.0',
  'FN:John Doe',
  'EMAIL:john@home.com',
  'TEL:1 (555) 111-2222',
  'END:VCARD',
].join('\n');

const EMAIL_DUP = [
  'BEGIN:VCARD',
  'VERSION:4.0',
  'FN:Ada L.',
  'EMAIL:ada@example.com',
  'END:VCARD',
  'BEGIN:VCARD',
  'VERSION:4.0',
  'FN:Ada Lovelace',
  'EMAIL:ADA@example.com',
  'TEL:+1-555-987-6543',
  'END:VCARD',
].join('\n');

test('vcard-deduplicate merges same-name contacts and unions emails', async ({ page }) => {
  await page.goto('/tools/vcard-deduplicate/');
  await page.fill('#in-data', TWO_JOHNS);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('BEGIN:VCARD', { timeout: 15000 });
  await expect(out).toContainText('EMAIL:john@work.com');
  await expect(out).toContainText('EMAIL:john@home.com');
  await expect(out).toHaveText(/BEGIN:VCARD[\s\S]*END:VCARD\s*$/);
  expect(((await out.textContent()) || '').match(/BEGIN:VCARD/g)).toHaveLength(1);
});

test('vcard-deduplicate can remove copies without merging', async ({ page }) => {
  await page.goto('/tools/vcard-deduplicate/');
  await page.fill('#in-data', TWO_JOHNS);
  await page.selectOption('#in-match_by', 'name');
  await page.uncheck('#in-merge');

  const text = (await page.locator('#tool-output').textContent({ timeout: 15000 })) || '';
  expect(text).toContain('EMAIL:john@work.com');
  expect(text).not.toContain('EMAIL:john@home.com');
  expect(text.match(/BEGIN:VCARD/g)).toHaveLength(1);
});

test('vcard-deduplicate supports email matching and deep links', async ({ page }) => {
  const data = encodeURIComponent(EMAIL_DUP);
  await page.goto(`/tools/vcard-deduplicate/?data=${data}&match_by=email&merge=true`);
  const text = (await page.locator('#tool-output').textContent({ timeout: 15000 })) || '';
  expect(text.match(/BEGIN:VCARD/g)).toHaveLength(1);
  expect(text).toContain('EMAIL:ada@example.com');
  expect(text).toContain('TEL:+1-555-987-6543');
});

test('vcard-deduplicate can force phone-only matching', async ({ page }) => {
  await page.goto('/tools/vcard-deduplicate/');
  await page.fill('#in-data', TWO_JOHNS);
  await page.selectOption('#in-match_by', 'phone');

  const text = (await page.locator('#tool-output').textContent({ timeout: 15000 })) || '';
  expect(text.match(/BEGIN:VCARD/g)).toHaveLength(1);
  expect(text).toContain('EMAIL:john@home.com');
});
