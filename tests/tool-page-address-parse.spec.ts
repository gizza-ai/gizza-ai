import { test, expect } from './fixtures';

test('address-parse page extracts a full US address', async ({ page }) => {
  await page.goto('/tools/address-parse/');
  await page.fill('#in-address', '123 Main St Apt 4B, Springfield, IL 62704, USA');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('House number: 123', { timeout: 15_000 });
  await expect(out).toContainText('Street:       Main St');
  await expect(out).toContainText('Unit:         Apt 4B');
  await expect(out).toContainText('City:         Springfield');
  await expect(out).toContainText('Region:       Illinois (IL)');
  await expect(out).toContainText('Postcode:     62704');
  await expect(out).toContainText('Country:      United States (US)');
});

test('address-parse deep-link country hint fills missing country', async ({ page }) => {
  await page.goto('/tools/address-parse/?country=US');
  await expect(page.locator('#in-country')).toHaveValue('US');
  await page.fill('#in-address', '350 Fifth Avenue, New York, NY 10118');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Street:       Fifth Avenue', { timeout: 15_000 });
  await expect(out).toContainText('City:         New York');
  await expect(out).toContainText('Region:       New York (NY)');
  await expect(out).toContainText('Postcode:     10118');
  await expect(out).toContainText('Country:      United States (US)');
});

test('address-parse parses UK and Canada examples', async ({ page }) => {
  await page.goto('/tools/address-parse/');
  await page.fill('#in-address', '221B Baker Street, London, NW1 6XE, United Kingdom');
  let out = page.locator('#tool-output');
  await expect(out).toContainText('House number: 221B', { timeout: 15_000 });
  await expect(out).toContainText('Street:       Baker Street');
  await expect(out).toContainText('Postcode:     NW1 6XE');
  await expect(out).toContainText('Country:      United Kingdom (GB)');

  await page.fill('#in-address', '100 Queen St W, Toronto, ON M5H 2N2, Canada');
  out = page.locator('#tool-output');
  await expect(out).toContainText('City:         Toronto', { timeout: 15_000 });
  await expect(out).toContainText('Region:       Ontario (ON)');
  await expect(out).toContainText('Postcode:     M5H 2N2');
  await expect(out).toContainText('Country:      Canada (CA)');
});

test('address-parse reports empty input clearly', async ({ page }) => {
  await page.goto('/tools/address-parse/');
  await page.fill('#in-address', '   ');
  await expect(page.locator('#tool-output')).toContainText('empty input', { timeout: 15_000 });
});
