import { test, expect } from './fixtures';

const SAMPLE_HAR = JSON.stringify({
  log: {
    version: '1.2',
    creator: { name: 'test', version: '1' },
    entries: [
      {
        request: {
          method: 'POST',
          url: 'https://example.test/login?token=SECRET123&page=2',
          headers: [
            { name: 'Cookie', value: 'sid=abcdef; theme=dark' },
            { name: 'Authorization', value: 'Bearer ey.secret.sig' },
            { name: 'Accept', value: 'application/json' },
          ],
          cookies: [{ name: 'sessionid', value: 'topsecret' }],
          queryString: [
            { name: 'token', value: 'SECRET123' },
            { name: 'page', value: '2' },
          ],
          postData: { text: 'user=ada&password=hunter2', params: [{ name: 'password', value: 'hunter2' }] },
        },
        response: {
          status: 200,
          headers: [{ name: 'Set-Cookie', value: 'sid=newsecret; HttpOnly' }],
          cookies: [{ name: 'sid', value: 'newsecret' }],
          content: { mimeType: 'application/json', text: '{"session":"privatetok"}' },
        },
      },
    ],
  },
});

async function outText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('har-redact default masks cookies, auth headers, query tokens, and response body', async ({ page }) => {
  await page.goto('/tools/har-redact/');
  await page.fill('#in-har', SAMPLE_HAR);
  await expect(page.locator('#tool-output')).toContainText('[REDACTED]', { timeout: 15000 });
  const redacted = JSON.parse(await outText(page));
  const entry = redacted.log.entries[0];
  expect(entry.request.headers[0].value).toBe('[REDACTED]');
  expect(entry.request.headers[1].value).toBe('[REDACTED]');
  expect(entry.request.headers[2].value).toBe('application/json');
  expect(entry.request.cookies[0].value).toBe('[REDACTED]');
  expect(entry.request.queryString[0].value).toBe('[REDACTED]');
  expect(entry.request.queryString[1].value).toBe('2');
  expect(entry.request.url).toBe('https://example.test/login?token=[REDACTED]&page=2');
  expect(entry.request.postData.text).toBe('user=ada&password=hunter2'); // request bodies default off
  expect(entry.response.headers[0].value).toBe('[REDACTED]');
  expect(entry.response.cookies[0].value).toBe('[REDACTED]');
  expect(entry.response.content.text).toBe('[REDACTED]');
});

test('har-redact deep-link summary honors non-default checkbox and enum values', async ({ page }) => {
  await page.goto('/tools/har-redact/?cookies=false&bodies=both&output=summary&placeholder=%5BSAFE%5D');
  await expect(page.locator('#in-cookies')).not.toBeChecked({ timeout: 15000 });
  await expect(page.locator('#in-bodies')).toHaveValue('both');
  await expect(page.locator('#in-output')).toHaveValue('summary');
  await expect(page.locator('#in-placeholder')).toHaveValue('[SAFE]');
  await page.fill('#in-har', SAMPLE_HAR);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('HAR redaction summary', { timeout: 15000 });
  await expect(out).toContainText('entries scanned: 1');
  await expect(out).toContainText('cookies redacted: 0');
  await expect(out).toContainText('body fields redacted: 3');
  await expect(out).toContainText('placeholder: [SAFE]');
});
