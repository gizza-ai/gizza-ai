import { test, expect } from './fixtures';

const BASIC = `Authorization: Basic ${Buffer.from('alice:wonderland').toString('base64')}`;
const b64url = (s: string) => Buffer.from(s).toString('base64url');
const JWT = `Bearer ${b64url('{"alg":"HS256","typ":"JWT"}')}.${b64url('{}')}.${b64url('sig')}`;
const DIGEST = 'Digest username="alice", realm="example.com", nonce="dcd98b7102dd2f0e", uri="/dir/index.html", qop=auth, nc=00000001, response="6629fae49393a05397450978507c4ef1"';

async function runWasm(
  page,
  header: string,
  format = 'json',
  maskCredentials = 'false',
  strict = 'false',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/authorization-header-decode/gizza_ai_authorization_header_decode_web.js');
    await mod.default('/tools/authorization-header-decode/gizza_ai_authorization_header_decode_web_bg.wasm');
    return mod.run(args.header, args.format, args.maskCredentials, args.strict);
  }, { header, format, maskCredentials, strict });
}

test('authorization-header-decode wasm decodes Basic credentials exactly', async ({ page }) => {
  await page.goto('/tools/authorization-header-decode/');
  await page.waitForSelector('#in-header');

  const decoded = JSON.parse(await runWasm(page, BASIC));
  expect(decoded).toMatchObject({
    header_name: 'Authorization',
    scheme: 'Basic',
    scheme_canonical: 'Basic',
    credentials_kind: 'token68',
  });
  expect(decoded.basic).toMatchObject({
    username: 'alice',
    password: 'wonderland',
    has_separator: true,
    valid_utf8: true,
    password_length: 10,
  });
});

test('authorization-header-decode wasm covers formats, masking, strict, bearer, digest, and cap', async ({ page }) => {
  await page.goto('/tools/authorization-header-decode/');
  await page.waitForSelector('#in-header');

  const text = await runWasm(page, BASIC, 'text', 'true');
  expect(text).toContain('scheme:          Basic');
  expect(text).toContain('username:        alice');
  expect(text).toContain('password:        ******** (10 chars)');

  const table = await runWasm(page, DIGEST, 'table');
  expect(table).toContain('| scheme          | Digest');
  expect(table).toContain('response');
  expect(table).toContain('6629fae49393a05397450978507c4ef1');

  const jwt = JSON.parse(await runWasm(page, JWT));
  expect(jwt.scheme_canonical).toBe('Bearer');
  expect(jwt.bearer).toMatchObject({ token_type: 'jwt', charset: 'base64url', segments: 3 });
  expect(jwt.bearer.segment_lengths).toEqual([36, 3, 4]);
  expect(jwt.bearer.jose_header).toMatchObject({ alg: 'HS256', typ: 'JWT' });

  await expect(runWasm(page, 'basic ' + btoa('alice:wonderland'), 'json', 'false', 'true'))
    .rejects.toThrow(/strict mode/);

  const atCap = 'Token ' + 'a'.repeat(8192 - 'Token '.length);
  await expect(runWasm(page, atCap)).resolves.toContain('"credentials_length": 8186');
  await expect(runWasm(page, `${atCap}x`)).rejects.toThrow(/limit is 8192/);
});

test('authorization-header-decode page renders decoded output from form controls', async ({ page }) => {
  await page.goto('/tools/authorization-header-decode/');
  await page.fill('#in-header', BASIC);
  await page.selectOption('#in-format', 'text');
  await page.check('#in-mask_credentials');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('scheme: Basic', { timeout: 15_000 });
  await expect(out).toContainText('username: alice');
  await expect(out).toContainText('password: ******** (10 chars)');
});

test('authorization-header-decode deep-link prefills controls and renders table output', async ({ page }) => {
  const params = new URLSearchParams({
    header: DIGEST,
    format: 'table',
    mask_credentials: 'true',
    strict: 'false',
  });

  await page.goto(`/tools/authorization-header-decode/?${params.toString()}`);
  await expect(page.locator('#in-header')).toHaveValue(DIGEST, { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('table');
  await expect(page.locator('#in-mask_credentials')).toBeChecked();
  await expect(page.locator('#in-strict')).not.toBeChecked();

  await expect(page.locator('#tool-output')).toContainText('scheme', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Digest');
  await expect(page.locator('#tool-output')).toContainText('response');
  await expect(page.locator('#tool-output')).toContainText('********');
});
