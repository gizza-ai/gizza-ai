import { test, expect } from './fixtures';

const LOGFMT = 'level=info msg="user signed in" user_id=42 ok=true';

async function runWasm(
  page: any,
  data: string,
  from = 'auto',
  to = 'json',
  delimiter = 'comma',
  detectTypes = 'true',
  pretty = 'false',
  flatten = 'true',
  keys = '',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/logfmt-converter/gizza_ai_logfmt_converter_web.js');
    await mod.default('/tools/logfmt-converter/gizza_ai_logfmt_converter_web_bg.wasm');
    return mod.run(
      args.data,
      args.from,
      args.to,
      args.delimiter,
      args.detectTypes,
      args.pretty,
      args.flatten,
      args.keys,
    );
  }, { data, from, to, delimiter, detectTypes, pretty, flatten, keys });
}

test('logfmt-converter wasm converts every advertised target exactly', async ({ page }) => {
  await page.goto('/tools/logfmt-converter/');

  await expect(runWasm(page, LOGFMT, 'logfmt', 'json'))
    .resolves.toBe('[{"level":"info","msg":"user signed in","user_id":42,"ok":true}]');
  await expect(runWasm(page, LOGFMT, 'logfmt', 'ndjson'))
    .resolves.toBe('{"level":"info","msg":"user signed in","user_id":42,"ok":true}');
  await expect(runWasm(page, LOGFMT, 'logfmt', 'csv'))
    .resolves.toBe('level,msg,user_id,ok\ninfo,user signed in,42,true');
  await expect(runWasm(page, '[{"level":"error","msg":"disk full","retries":2}]', 'json', 'logfmt'))
    .resolves.toBe('level=error msg="disk full" retries=2');
});

test('logfmt-converter page computes exact JSON output from the form', async ({ page }) => {
  await page.goto('/tools/logfmt-converter/');
  await page.fill('#in-data', LOGFMT);
  await page.selectOption('#in-from', 'logfmt');
  await page.selectOption('#in-to', 'json');
  await page.uncheck('#in-pretty');

  await expect(page.locator('#tool-output')).toHaveText('[{"level":"info","msg":"user signed in","user_id":42,"ok":true}]', { timeout: 15_000 });
});

test('logfmt-converter deep link covers field order and checkbox state', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'n=42 ok=true msg=hi',
    from: 'logfmt',
    to: 'json',
    delimiter: 'comma',
    detect_types: 'false',
    pretty: 'false',
    flatten: 'true',
    keys: 'ok,n',
  });
  await page.goto(`/tools/logfmt-converter/?${params.toString()}`);

  await expect(page.locator('#in-from')).toHaveValue('logfmt', { timeout: 15_000 });
  await expect(page.locator('#in-to')).toHaveValue('json');
  await expect(page.locator('#in-detect_types')).not.toBeChecked();
  await expect(page.locator('#in-keys')).toHaveValue('ok,n');
  await expect(page.locator('#tool-output')).toHaveText('[{"ok":"true","n":"42"}]', { timeout: 15_000 });
});

test('logfmt-converter covers CSV delimiter, flatten off, cap boundary, and CLI example', async ({ page }) => {
  await page.goto('/tools/logfmt-converter/');

  await expect(runWasm(page, 'a;b\n1;2', 'csv', 'csv', 'semicolon'))
    .resolves.toBe('a;b\n1;2');
  await expect(runWasm(page, '[{"user":{"id":7}}]', 'json', 'logfmt', 'comma', 'true', 'false', 'false'))
    .resolves.toBe('user="{\\"id\\":7}"');

  const atCap = 'k=1\n'.repeat(250_000);
  await expect(runWasm(page, atCap, 'logfmt', 'ndjson'))
    .resolves.toContain('{"k":1}');
  await expect(runWasm(page, `${atCap}x`, 'logfmt', 'ndjson'))
    .rejects.toThrow(/maximum is 1000000/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool logfmt-converter');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
