import { test, expect } from './fixtures';

// /tools/http-header-parser/ parses a raw HTTP header block into a
// case-normalized JSON map in-browser (pure wasm). Output is #tool-output.

test('http-header-parser maps a response head with canonical casing', async ({ page }) => {
  await page.goto('/tools/http-header-parser/');
  await page.fill(
    '#in-headers',
    'HTTP/1.1 200 OK\nContent-Type: text/html; charset=utf-8\nCache-Control: max-age=60\nSet-Cookie: id=1; Path=/\nSet-Cookie: sess=xyz; HttpOnly',
  );
  const out = page.locator('#tool-output');
  const text = (await out.textContent({ timeout: 15000 })) ?? '';
  const parsed = JSON.parse(text);
  expect(parsed.kind).toBe('response');
  expect(parsed.start_line).toBe('HTTP/1.1 200 OK');
  expect(parsed.case).toBe('canonical');
  expect(parsed.duplicate_policy).toBe('combine');
  expect(parsed.headers['Content-Type']).toBe('text/html; charset=utf-8');
  expect(parsed.headers['Cache-Control']).toBe('max-age=60');
  // Set-Cookie is never comma-joined under combine (RFC 6265).
  expect(parsed.headers['Set-Cookie']).toEqual(['id=1; Path=/', 'sess=xyz; HttpOnly']);
  expect(parsed.count).toBe(3);
  expect(parsed.line_count).toBe(4);
  expect(parsed.duplicates).toEqual(['Set-Cookie']);
});

test('http-header-parser combine joins repeated custom headers, folds case-insensitively', async ({ page }) => {
  await page.goto('/tools/http-header-parser/');
  await page.fill('#in-headers', 'x-custom: a\nX-CUSTOM: b\nAccept: */*');
  await page.selectOption('#in-case', 'canonical');
  await page.selectOption('#in-duplicates', 'combine');
  const out = page.locator('#tool-output');
  const parsed = JSON.parse((await out.textContent({ timeout: 15000 })) ?? '');
  expect(parsed.kind).toBe('headers');
  expect(parsed.headers['X-Custom']).toBe('a, b');
  expect(parsed.headers['Accept']).toBe('*/*');
  expect(parsed.duplicates).toEqual(['X-Custom']);
});

test('http-header-parser list policy keeps every value as an array', async ({ page }) => {
  await page.goto('/tools/http-header-parser/');
  await page.fill('#in-headers', 'A: 1\nA: 2\nB: 3');
  await page.selectOption('#in-case', 'lower');
  await page.selectOption('#in-duplicates', 'list');
  const parsed = JSON.parse((await page.locator('#tool-output').textContent({ timeout: 15000 })) ?? '');
  expect(parsed.case).toBe('lower');
  expect(parsed.duplicate_policy).toBe('list');
  expect(parsed.headers['a']).toEqual(['1', '2']);
  expect(parsed.headers['b']).toEqual(['3']);
});

test('http-header-parser first and last policies keep a single occurrence', async ({ page }) => {
  await page.goto('/tools/http-header-parser/');
  await page.fill('#in-headers', 'A: 1\nA: 2');
  await page.selectOption('#in-duplicates', 'first');
  let parsed = JSON.parse((await page.locator('#tool-output').textContent({ timeout: 15000 })) ?? '');
  expect(parsed.headers['A']).toBe('1');
  await page.selectOption('#in-duplicates', 'last');
  parsed = JSON.parse((await page.locator('#tool-output').textContent({ timeout: 15000 })) ?? '');
  expect(parsed.headers['A']).toBe('2');
});

test('http-header-parser upper and original casing render the chosen style', async ({ page }) => {
  await page.goto('/tools/http-header-parser/');
  await page.fill('#in-headers', 'cOnTeNt-TyPe: text/plain');
  await page.selectOption('#in-case', 'upper');
  let parsed = JSON.parse((await page.locator('#tool-output').textContent({ timeout: 15000 })) ?? '');
  expect(Object.keys(parsed.headers)[0]).toBe('CONTENT-TYPE');
  await page.selectOption('#in-case', 'original');
  parsed = JSON.parse((await page.locator('#tool-output').textContent({ timeout: 15000 })) ?? '');
  expect(Object.keys(parsed.headers)[0]).toBe('cOnTeNt-TyPe');
});

test('http-header-parser honours a query-param deep link', async ({ page }) => {
  await page.goto('/tools/http-header-parser/?headers=GET%20%2Fi%3Fq%3D1%20HTTP%2F1.1%0AHost%3A%20example.com&case=lower&duplicates=list');
  await expect(page.locator('#in-case')).toHaveValue('lower');
  await expect(page.locator('#in-duplicates')).toHaveValue('list');
  const parsed = JSON.parse((await page.locator('#tool-output').textContent({ timeout: 15000 })) ?? '');
  expect(parsed.kind).toBe('request');
  expect(parsed.start_line).toBe('GET /i?q=1 HTTP/1.1');
  expect(parsed.headers['host']).toEqual(['example.com']);
});
