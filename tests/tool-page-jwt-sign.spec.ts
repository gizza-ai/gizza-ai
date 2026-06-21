import { test, expect } from './fixtures';

// /tools/jwt-sign/ builds and signs a JWT in-browser (pure wasm).
// payload + secret + header are multiline <textarea>; algorithm is a <select>.

function b64urlDecode(s: string): string {
  s = s.replace(/-/g, '+').replace(/_/g, '/');
  while (s.length % 4) s += '=';
  return Buffer.from(s, 'base64').toString('utf-8');
}

test('jwt-sign page produces a 3-part HS256 token', async ({ page }) => {
  await page.goto('/tools/jwt-sign/');
  await page.fill('#in-payload', '{"sub":"1234567890","name":"Ada Lovelace"}');
  await page.fill('#in-secret', 'my-shared-secret');
  await page.selectOption('#in-algorithm', 'HS256');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const jwt = (await out.textContent())!.trim();
  const parts = jwt.split('.');
  expect(parts.length).toBe(3);
  // header decodes to an object with alg HS256 / typ JWT
  const header = JSON.parse(b64urlDecode(parts[0]));
  expect(header.alg).toBe('HS256');
  expect(header.typ).toBe('JWT');
  // payload round-trips
  const payload = JSON.parse(b64urlDecode(parts[1]));
  expect(payload.sub).toBe('1234567890');
  expect(payload.name).toBe('Ada Lovelace');
});

test('jwt-sign page reflects the chosen algorithm in the header', async ({ page }) => {
  await page.goto('/tools/jwt-sign/');
  await page.fill('#in-payload', '{"a":1}');
  await page.fill('#in-secret', 'k');
  await page.selectOption('#in-algorithm', 'HS512');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const jwt = (await out.textContent())!.trim();
  const header = JSON.parse(b64urlDecode(jwt.split('.')[0]));
  expect(header.alg).toBe('HS512');
});

test('jwt-sign page errors clearly on a non-object payload', async ({ page }) => {
  await page.goto('/tools/jwt-sign/');
  await page.fill('#in-payload', 'not json');
  await page.fill('#in-secret', 'k');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('JSON', { timeout: 15000 });
});
