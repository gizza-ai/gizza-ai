import { test, expect } from './fixtures';

async function setValue(page: import('@playwright/test').Page, selector: string, value: string) {
  await page.locator(selector).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('form-field-validator validates a full US form and masks card output', async ({ page }) => {
  await page.goto('/tools/form-field-validator/');
  await setValue(page, '#in-fields', 'email: John.Doe@Example.COM\nphone: (415) 555-2671\nzip: 90210\nwebsite: https://example.com\ncard: 4111 1111 1111 1111');
  await page.selectOption('#in-country', 'US');
  await setValue(page, '#in-required', 'email, phone, zip');
  await setValue(page, '#in-rules', 'zip: postal-code\ncard: credit-card');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('VALID — 5 field(s) checked: 5 passed, 0 failed. Country: US (United States).', { timeout: 15_000 });
  const text = await out.textContent();
  expect(text).toContain('OK   email [email] = John.Doe@example.com');
  expect(text).toContain('phone [phone]');
  expect(text).toContain('+141');
  expect(text).toContain('2671');
  expect(text).toContain('OK   card [credit-card] = ************1111 (Visa)');
  expect(text).not.toContain('4111 1111 1111 1111');
});

test('form-field-validator deep-links JSON output and required errors', async ({ page }) => {
  const qs = new URLSearchParams({
    fields: 'email: john@\nphone: 555-12\nzip: 9021',
    country: 'US',
    required: 'email, phone, zip, website',
    rules: 'zip: postal-code',
    normalize: 'true',
    mask_sensitive: 'true',
    output: 'json',
  });
  await page.goto(`/tools/form-field-validator/?${qs.toString()}`);

  await expect(page.locator('#in-country')).toHaveValue('US');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-fields')).toHaveValue('email: john@\nphone: 555-12\nzip: 9021');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"valid": false', { timeout: 15_000 });
  await expect(out).toContainText('"failed": 4');
  await expect(out).toContainText('"name": "website"');
  await expect(out).toContainText('"expected_format": "NNNNN or NNNNN-NNNN (N = digit)"');
});

test('form-field-validator covers country, output and checkbox value variants', async ({ page }) => {
  await page.goto('/tools/form-field-validator/');
  await setValue(page, '#in-fields', 'postcode: SW1A 1AA\nmobile: +44 20 7946 0958\ncard: 3782 822463 10005');
  await page.selectOption('#in-country', 'GB');
  await setValue(page, '#in-required', '*');
  await setValue(page, '#in-rules', 'postcode: postal-code\ncard: credit-card');
  await page.locator('#in-mask_sensitive').uncheck();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Country: GB (United Kingdom)', { timeout: 15_000 });
  const text = await out.textContent();
  expect(text).toContain('OK   postcode [postal-code] = SW1A 1AA');
  expect(text).toContain('mobile [phone]');
  expect(text).toContain('+442');
  expect(text).toContain('0958');
  expect(text).toContain('OK   card [credit-card] = 378282246310005 (American Express; was "3782 822463 10005")');
});
