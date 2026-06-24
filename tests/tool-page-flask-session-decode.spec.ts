import { test, expect } from './fixtures';

// /tools/flask-session-decode/ decodes a Flask / itsdangerous session cookie into
// JSON in-browser (pure wasm). The cookie field is a multiline <textarea>.
// Output is a pretty-printed JSON object {payload, compressed, timestamp, ...}.

function b64url(buf: Buffer): string {
  return buf
    .toString('base64')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}

// itsdangerous epoch = 2011-01-01 = Unix 1293840000.
function makeCookie(payload: object, unixTs: number): string {
  const p = b64url(Buffer.from(JSON.stringify(payload), 'utf-8'));
  const off = unixTs - 1293840000;
  // big-endian, leading zero bytes trimmed (like itsdangerous)
  let hex = off.toString(16);
  if (hex.length % 2) hex = '0' + hex;
  const ts = b64url(Buffer.from(hex, 'hex'));
  return `${p}.${ts}.fakeSIGNATURE`;
}

test('flask-session-decode page decodes an uncompressed session', async ({ page }) => {
  await page.goto('/tools/flask-session-decode/');
  await page.fill('#in-input', makeCookie({ logged_in: true, user: 'alice' }, 1700000000));
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const report = JSON.parse((await out.textContent())!.trim());
  expect(report.compressed).toBe(false);
  expect(report.payload.user).toBe('alice');
  expect(report.payload.logged_in).toBe(true);
  expect(report.timestamp).toBe(1700000000);
  expect(report.timestamp_iso).toBe('2023-11-14T22:13:20Z');
  expect(report.signature_verified).toBe(false);
});

test('flask-session-decode page strips a full session=... fragment', async ({ page }) => {
  await page.goto('/tools/flask-session-decode/');
  const inner = makeCookie({ user: 'bob' }, 1700000000);
  await page.fill('#in-input', `session="${inner}"; Path=/; HttpOnly; SameSite=Lax`);
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const report = JSON.parse((await out.textContent())!.trim());
  expect(report.payload.user).toBe('bob');
});

test('flask-session-decode page errors clearly on a non-cookie input', async ({ page }) => {
  await page.goto('/tools/flask-session-decode/');
  await page.fill('#in-input', 'onlyonesegment');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('payload.timestamp.signature', { timeout: 15000 });
});
