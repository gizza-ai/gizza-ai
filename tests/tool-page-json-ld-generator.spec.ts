import { test, expect } from './fixtures';

test('json-ld-generator builds an Article with nested author and keywords', async ({ page }) => {
  await page.goto('/tools/json-ld-generator/');

  // Default schema_type is Article.
  await page.fill('#in-fields', 'headline = Hello World\nauthor.name = Jane Doe\nkeywords = seo, json-ld');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"@type": "Article"', { timeout: 15000 });
  await expect(out).toContainText('"headline": "Hello World"');
  await expect(out).toContainText('"@type": "Person"');
  await expect(out).toContainText('"name": "Jane Doe"');
  // keywords splits into a JSON array.
  await expect(out).toContainText('"seo"');
  await expect(out).toContainText('"json-ld"');

  // The output is valid JSON-LD.
  const text = (await out.textContent()) ?? '';
  const parsed = JSON.parse(text);
  expect(parsed['@context']).toBe('https://schema.org');
  expect(parsed['author']['@type']).toBe('Person');
  expect(parsed['keywords']).toEqual(['seo', 'json-ld']);
});

test('json-ld-generator Product offers.price becomes a JSON number', async ({ page }) => {
  await page.goto('/tools/json-ld-generator/');
  await page.selectOption('#in-schema_type', 'Product');
  await page.fill('#in-fields', 'name = Widget\noffers.price = 19.99\noffers.priceCurrency = USD');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"@type": "Product"', { timeout: 15000 });
  const parsed = JSON.parse((await out.textContent()) ?? '');
  expect(parsed['offers']['@type']).toBe('Offer');
  expect(parsed['offers']['price']).toBe(19.99); // number, not string
  expect(parsed['offers']['priceCurrency']).toBe('USD');
});

test('json-ld-generator FAQPage expands Q&A and the wrap-script checkbox adds a tag', async ({ page }) => {
  await page.goto('/tools/json-ld-generator/');
  await page.selectOption('#in-schema_type', 'FAQPage');
  await page.fill('#in-faq', 'Is it free? | Yes.\nIs it private? :: In your browser.');
  await page.check('#in-wrap_script');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<script type="application/ld+json">', { timeout: 15000 });
  await expect(out).toContainText('"@type": "FAQPage"');
  await expect(out).toContainText('"@type": "Question"');
  await expect(out).toContainText('"@type": "Answer"');
  await expect(out).toContainText('"Is it free?"');
});

test('json-ld-generator query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/json-ld-generator/?schema_type=WebSite&fields=' +
      encodeURIComponent('name = gizza\nurl = https://gizza.ai'),
  );
  await expect(page.locator('#in-schema_type')).toHaveValue('WebSite', { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"@type": "WebSite"', { timeout: 15000 });
  await expect(out).toContainText('"url": "https://gizza.ai"');
});
