import { test, expect } from './fixtures';

const validHar = JSON.stringify({
  log: {
    version: '1.2',
    creator: { name: 'WebInspector', version: '537.36' },
    entries: [
      {
        startedDateTime: '2024-01-01T00:00:00.000Z',
        time: 100,
        request: {
          method: 'GET',
          url: 'https://example.com/',
          httpVersion: 'HTTP/1.1',
          cookies: [],
          headers: [],
          queryString: [],
          headersSize: -1,
          bodySize: 0,
        },
        response: {
          status: 200,
          statusText: 'OK',
          httpVersion: 'HTTP/1.1',
          cookies: [],
          headers: [],
          content: { size: 12, mimeType: 'text/html' },
          redirectURL: '',
          headersSize: -1,
          bodySize: 12,
        },
        cache: {},
        timings: { send: 10, wait: 80, receive: 10 },
      },
    ],
  },
});

const timingMismatchHar = validHar.replace('"time":100', '"time":250');
const missingMethodHar = validHar.replace('"method":"GET",', '');

test('har-validator emits an exact valid report', async ({ page }) => {
  await page.goto('/tools/har-validator/');
  await page.fill('#in-har', validHar);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('✓ Valid HAR (HTTP Archive 1.2)', { timeout: 15000 });
  expect((await out.textContent())?.trim()).toBe(
    [
      '✓ Valid HAR (HTTP Archive 1.2)',
      '',
      'Summary',
      '  version:  1.2',
      '  creator:  WebInspector 537.36',
      '  pages:    0',
      '  entries:  1',
      '',
      'No structural errors found.',
    ].join('\n'),
  );
});

test('har-validator reports structural errors with JSON paths', async ({ page }) => {
  await page.goto('/tools/har-validator/');
  await page.fill('#in-har', missingMethodHar);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('✗ Invalid HAR (HTTP Archive 1.2)', { timeout: 15000 });
  await expect(out).toContainText('Errors (1)');
  await expect(out).toContainText('log.entries[0].request: missing required field "method"');
});

test('har-validator warns on timing mismatch when the checkbox is enabled', async ({ page }) => {
  await page.goto('/tools/har-validator/');
  await page.fill('#in-har', timingMismatchHar);
  await page.check('#in-check_timings');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('✓ Valid HAR (HTTP Archive 1.2)', { timeout: 15000 });
  await expect(out).toContainText('Warnings (1)');
  await expect(out).toContainText('entry time (250) ≠ sum of phases (100); difference 150 ms');
});

test('har-validator deep link can disable timing warnings', async ({ page }) => {
  const params = new URLSearchParams({
    har: timingMismatchHar,
    check_timings: 'false',
  });
  await page.goto(`/tools/har-validator/?${params.toString()}`);

  await expect(page.locator('#in-har')).toHaveValue(timingMismatchHar, { timeout: 15000 });
  await expect(page.locator('#in-check_timings')).not.toBeChecked();
  const out = page.locator('#tool-output');
  await expect(out).toContainText('✓ Valid HAR (HTTP Archive 1.2)', { timeout: 15000 });
  await expect(out).not.toContainText('Warnings');
});
