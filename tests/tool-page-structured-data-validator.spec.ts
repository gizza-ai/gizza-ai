import { test, expect } from './fixtures';

const validJsonLd = '<script type="application/ld+json">{"@context":"https://schema.org","@type":"Article","headline":"Hello","author":{"@type":"Person","name":"Jane"}}</script>';
const invalidMixed = '<script type="application/ld+json">{"@type":"Thing","name":"x"}</script><span itemprop="orphan">oops</span>';
const microdata = '<div itemscope itemtype="https://schema.org/Product"><span itemprop="name">Widget</span><span itemprop="price">9.99</span></div>';

async function setHtml(page: any, value: string) {
  await page.$eval(
    '#in-html',
    (el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value,
  );
}

test('structured-data-validator reports valid JSON-LD Article', async ({ page }) => {
  await page.goto('/tools/structured-data-validator/');
  await setHtml(page, validJsonLd);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Status: VALID', { timeout: 15000 });
  await expect(out).toContainText('[json-ld] Article');
  await expect(out).toContainText('headline');
});

test('structured-data-validator flags missing context and orphan microdata', async ({ page }) => {
  await page.goto('/tools/structured-data-validator/');
  await setHtml(page, invalidMixed);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Status: INVALID', { timeout: 15000 });
  await expect(out).toContainText('missing "@context"');
  await expect(out).toContainText('itemprop');
});

test('structured-data-validator json format emits machine-readable summary', async ({ page }) => {
  await page.goto('/tools/structured-data-validator/');
  await page.selectOption('#in-format', 'json');
  await setHtml(page, microdata);
  const text = await page.locator('#tool-output').innerText({ timeout: 15000 });
  const data = JSON.parse(text);
  expect(data.valid).toBe(true);
  expect(data.counts.microdata).toBe(1);
  expect(data.items[0].types).toContain('https://schema.org/Product');
});

test('structured-data-validator query-param deep-link pre-fills and computes', async ({ page }) => {
  await page.goto('/tools/structured-data-validator/?html=' + encodeURIComponent(validJsonLd) + '&format=json');
  await expect(page.locator('#in-html')).toHaveValue(validJsonLd, { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  const text = await page.locator('#tool-output').innerText({ timeout: 15000 });
  const data = JSON.parse(text);
  expect(data.counts.jsonld).toBe(1);
  expect(data.valid).toBe(true);
});
