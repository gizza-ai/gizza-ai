import { test, expect } from './fixtures';

const COOKIE_HEADER = 'Cookie: sessionid=abc123; theme=dark; redirect=%2Faccount';
const SET_COOKIE = 'Set-Cookie: sid=abc123; Domain=example.com; Path=/; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Secure; HttpOnly; SameSite=Lax';
const MULTI_SET_COOKIE = 'Set-Cookie: __Host-id=1; Path=/; Secure; HttpOnly; SameSite=Strict\nSet-Cookie: tracker=xyz; Domain=.example.com; Max-Age=0; SameSite=None';

async function runWasm(
  page,
  cookie: string,
  mode = 'auto',
  format = 'json',
  decode = 'true',
  rawAttributes = 'false',
  warnings = 'true',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/cookie-parser/gizza_ai_cookie_parser_web.js');
    await mod.default('/tools/cookie-parser/gizza_ai_cookie_parser_web_bg.wasm');
    return mod.run(
      args.cookie,
      args.mode,
      args.format,
      args.decode,
      args.rawAttributes,
      args.warnings,
    );
  }, { cookie, mode, format, decode, rawAttributes, warnings });
}

test('cookie-parser wasm parses a Cookie header and decodes values exactly', async ({ page }) => {
  await page.goto('/tools/cookie-parser/');
  const out = await runWasm(page, COOKIE_HEADER);
  expect(out).toBe(`{
  "cookies": [
    {
      "name": "sessionid",
      "size": 16,
      "value": "abc123",
      "warnings": []
    },
    {
      "name": "theme",
      "size": 10,
      "value": "dark",
      "warnings": []
    },
    {
      "name": "redirect",
      "size": 19,
      "value": "/account",
      "warnings": []
    }
  ],
  "count": 3,
  "mode": "cookie"
}`);
});

test('cookie-parser wasm covers advertised enum choices and checkbox states', async ({ page }) => {
  await page.goto('/tools/cookie-parser/');

  await expect(runWasm(page, SET_COOKIE, 'auto', 'table'))
    .resolves.toContain('sid   abc123  10    example.com  /     2015-10-21T07:28:00Z');

  await expect(runWasm(page, MULTI_SET_COOKIE, 'set-cookie', 'markdown'))
    .resolves.toContain('- `tracker` — SameSite=None without Secure');

  const csvRaw = await runWasm(page, SET_COOKIE, 'set-cookie', 'csv', 'false', 'true', 'false');
  expect(csvRaw).toContain('Name,Value,Size,Domain,Path,Expires,Max-Age,Secure,HttpOnly,SameSite,Priority,Partitioned,Attributes');
  expect(csvRaw).toContain('sid,abc123,10,example.com,/,2015-10-21T07:28:00Z,,yes,yes,Lax,,no,');
  expect(csvRaw).toContain('Domain=example.com; Path=/; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Secure; HttpOnly; SameSite=Lax');

  await expect(runWasm(page, 'Cookie: bad=%2Fpath', 'cookie', 'json', 'false'))
    .resolves.toContain('"value": "%2Fpath"');
});

test('cookie-parser page renders exact output and deep-link prefills params', async ({ page }) => {
  await page.goto('/tools/cookie-parser/');
  await page.fill('#in-cookie', SET_COOKIE);
  await page.selectOption('#in-format', 'table');
  await expect(page.locator('#tool-output')).toHaveText(`1 cookie (Set-Cookie header)

Name  Value   Size  Domain       Path  Expires               Max-Age  Secure  HttpOnly  SameSite  Priority  Partitioned
----  ------  ----  -----------  ----  --------------------  -------  ------  --------  --------  --------  -----------
sid   abc123  10    example.com  /     2015-10-21T07:28:00Z  -        yes     yes       Lax       -         no`, { timeout: 15_000 });

  const qs =
    '?cookie=' + encodeURIComponent(COOKIE_HEADER) +
    '&mode=cookie' +
    '&format=json' +
    '&decode=true' +
    '&raw_attributes=false' +
    '&warnings=true';
  await page.goto('/tools/cookie-parser/' + qs);
  await expect(page.locator('#in-cookie')).toHaveValue(COOKIE_HEADER, { timeout: 15_000 });
  await expect(page.locator('#in-mode')).toHaveValue('cookie');
  await expect(page.locator('#tool-output')).toContainText('"redirect"', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"/account"');
});
