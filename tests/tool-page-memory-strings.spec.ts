import { test, expect } from './fixtures';

async function setDump(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-dump').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('memory-strings categorizes a text dump with exact output', async ({ page }) => {
  await page.goto('/tools/memory-strings/');
  await setDump(
    page,
    'GET http://evil.example.com/a from 203.0.113.5 open C:\\Windows\\System32\\cmd.exe key HKLM\\Software\\Run mail bad@phish.net host cdn.badsite.io',
  );
  await page.selectOption('#in-input_format', 'text');
  await page.selectOption('#in-encoding', 'ascii');
  await page.locator('#in-min_length').fill('3');
  await page.locator('#in-categories').fill('url,ipv4,path,registry,email,domain');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Extracted 1 printable string', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'Extracted 1 printable string — 6 categorized items\n\n' +
      'URLs (1):\n  http://evil.example.com/a\n\n' +
      'IPv4 addresses (1):\n  203.0.113.5\n\n' +
      'Emails (1):\n  bad@phish.net\n\n' +
      'Domains (1):\n  cdn.badsite.io\n\n' +
      'File paths (1):\n  C:\\Windows\\System32\\cmd.exe\n\n' +
      'Registry keys (1):\n  HKLM\\Software\\Run',
  );
});

test('memory-strings deep-link decodes hex input', async ({ page }) => {
  const dump = '56 69 73 69 74 20 68 74 74 70 3a 2f 2f 78 2e 69 6f 20 6e 6f 77';
  const qs = new URLSearchParams({ dump, input_format: 'hex', encoding: 'ascii', min_length: '4', categories: 'url' });
  await page.goto(`/tools/memory-strings/?${qs.toString()}`);

  await expect(page.locator('#in-dump')).toHaveValue(dump);
  await expect(page.locator('#in-input_format')).toHaveValue('hex');
  await expect(page.locator('#in-encoding')).toHaveValue('ascii');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText('Extracted 1 printable string — 1 categorized item\n\nURLs (1):\n  http://x.io', { timeout: 15_000 });
});

test('memory-strings recovers UTF-16LE and defangs indicators', async ({ page }) => {
  await page.goto('/tools/memory-strings/');
  await setDump(page, '68 00 74 00 74 00 70 00 73 00 3a 00 2f 00 2f 00 63 00 32 00 2e 00 65 00 76 00 69 00 6c 00 2e 00 65 00 78 00 61 00 6d 00 70 00 6c 00 65 00 2f 00 67 00 61 00 74 00 65 00');
  await page.selectOption('#in-input_format', 'hex');
  await page.selectOption('#in-encoding', 'utf16le');
  await page.locator('#in-categories').fill('url');
  await page.locator('#in-defang').check();

  const out = page.locator('#tool-output');
  await expect(out).toHaveText('Extracted 1 printable string — 1 categorized item\n\nURLs (1):\n  hxxps[://]c2[.]evil[.]example/gate', { timeout: 15_000 });
});

test('memory-strings honors the max min-length boundary', async ({ page }) => {
  await page.goto('/tools/memory-strings/');
  await setDump(page, 'http://x.io');
  await page.locator('#in-min_length').fill('1024');
  await page.locator('#in-categories').fill('url');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText('Extracted 0 printable strings — 0 categorized items\n\nURLs (0):\n  (none)', { timeout: 15_000 });
});
