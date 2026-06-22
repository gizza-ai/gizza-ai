import { test, expect } from './fixtures';

// /tools/extract-domains/ extracts hostnames + registrable domains in-browser (pure wasm).
test('extract-domains lists hostnames and registrable domains', async ({ page }) => {
  await page.goto('/tools/extract-domains/');
  await page.fill(
    '#in-text',
    'visit https://www.example.com/x and mail jo@mail.example.co.uk, also sub.test.org. and 1.2.3.4 ignored',
  );
  await page.selectOption('#in-mode', 'both');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('3 unique hostname(s)', { timeout: 15000 });
  await expect(out).toContainText('www.example.com');
  await expect(out).toContainText('mail.example.co.uk');
  await expect(out).toContainText('3 unique registrable domain(s)');
  await expect(out).toContainText('example.co.uk');
  // IP addresses are not domains.
  await expect(out).not.toContainText('1.2.3.4');
});

test('extract-domains registrable mode dedupes to apex domains', async ({ page }) => {
  await page.goto('/tools/extract-domains/');
  await page.fill('#in-text', 'a.example.com b.example.com a.example.com shop.foo.gov.uk');
  await page.selectOption('#in-mode', 'registrable');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 unique registrable domain(s)', { timeout: 15000 });
  await expect(out).toContainText('example.com');
  await expect(out).toContainText('foo.gov.uk');
});

test('extract-domains sorts alphabetically when checked', async ({ page }) => {
  await page.goto('/tools/extract-domains/');
  await page.fill('#in-text', 'zebra.com apple.com mango.com');
  await page.selectOption('#in-mode', 'registrable');
  await page.check('#in-sort');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('apple.com', { timeout: 15000 });
  // apple should appear before zebra in sorted output.
  const text = (await out.textContent()) ?? '';
  expect(text.indexOf('apple.com')).toBeLessThan(text.indexOf('zebra.com'));
});
