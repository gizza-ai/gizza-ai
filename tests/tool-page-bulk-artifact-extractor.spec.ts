import { test, expect } from './fixtures';

const INPUT = 'Contact alice@example.com, see https://data.example.org/path, server 203.0.113.7, call +1 415-555-0132, card 4111 1111 1111 1111.';

async function setText(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-text').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('bulk-artifact-extractor extracts real artifacts as table output', async ({ page }) => {
  await page.goto('/tools/bulk-artifact-extractor/');
  await setText(page, INPUT);
  await page.locator('#in-kinds').fill('all');
  await page.locator('#in-context').fill('24');
  await page.locator('#in-limit').fill('1000');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Bulk artifact extractor · 5 artifacts', { timeout: 15_000 });
  await expect(out).toContainText('| email | alice@example.com | 8 |');
  await expect(out).toContainText('| url | https://data.example.org/path |');
  await expect(out).toContainText('| ipv4 | 203.0.113.7 |');
  await expect(out).toContainText('| phone | +1 415-555-0132 |');
  await expect(out).toContainText('| credit_card | 4111 1111 1111 1111 |');
});

test('bulk-artifact-extractor deep-link filters to email and ipv4 JSON', async ({ page }) => {
  const qs = new URLSearchParams({
    text: 'alice@example.com logged in from 203.0.113.7; bob@example.net from 10.0.0.5.',
    kinds: 'email,ipv4',
    output: 'json',
    context: '12',
    limit: '3',
  });
  await page.goto(`/tools/bulk-artifact-extractor/?${qs.toString()}`);

  await expect(page.locator('#in-text')).toHaveValue('alice@example.com logged in from 203.0.113.7; bob@example.net from 10.0.0.5.');
  await expect(page.locator('#in-kinds')).toHaveValue('email,ipv4');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-limit')).toHaveValue('3');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"kind": "email"', { timeout: 15_000 });
  await expect(out).toContainText('"value": "alice@example.com"');
  await expect(out).toContainText('"kind": "ipv4"');
  await expect(out).toContainText('"value": "203.0.113.7"');
  await expect(out).not.toContainText('"kind": "domain"');
});
