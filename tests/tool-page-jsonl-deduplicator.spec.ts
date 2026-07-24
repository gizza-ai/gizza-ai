import { test, expect } from './fixtures';

test('jsonl-deduplicator page removes whole-line duplicates and keeps first occurrence', async ({ page }) => {
  await page.goto('/tools/jsonl-deduplicator/');
  await page.fill('#in-data', '{"a":1}\n{"a":2}\n{"a":1}');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('{"a":2}', { timeout: 15000 });
  expect(await out.textContent()).toBe('{"a":1}\n{"a":2}');
});

test('jsonl-deduplicator page supports key fields, keep last, and deep links', async ({ page }) => {
  const data = '{"id":1,"v":"a"}\n{"id":2,"v":"b"}\n{"id":1,"v":"c"}';
  const qs =
    '?data=' + encodeURIComponent(data) +
    '&keys=id' +
    '&keep=last' +
    '&ignore_case=false' +
    '&on_invalid=error';
  await page.goto('/tools/jsonl-deduplicator/' + qs);

  await expect(page.locator('#in-keys')).toHaveValue('id', { timeout: 15000 });
  await expect(page.locator('#in-keep')).toHaveValue('last');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('{"id":1,"v":"c"}', { timeout: 15000 });
  expect(await out.textContent()).toBe('{"id":2,"v":"b"}\n{"id":1,"v":"c"}');
});

test('jsonl-deduplicator page handles invalid JSON according to the selected policy', async ({ page }) => {
  await page.goto('/tools/jsonl-deduplicator/');
  await page.fill('#in-data', '{"id":1}\nnot json\n{"id":2}\n{"id":1}');
  await page.fill('#in-keys', 'id');

  await expect(page.locator('#tool-output')).toContainText('line 2: invalid JSON', { timeout: 15000 });

  await page.selectOption('#in-on_invalid', 'skip');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('{"id":2}', { timeout: 15000 });
  expect(await out.textContent()).toBe('{"id":1}\n{"id":2}');
});

test('jsonl-deduplicator page honors non-default ignore-case matching', async ({ page }) => {
  await page.goto('/tools/jsonl-deduplicator/');
  await page.fill('#in-data', '{"email":"A@X.COM"}\n{"email":"a@x.com"}');
  await page.fill('#in-keys', 'email');
  await page.check('#in-ignore_case');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('{"email":"A@X.COM"}', { timeout: 15000 });
  expect(await out.textContent()).toBe('{"email":"A@X.COM"}');
});
