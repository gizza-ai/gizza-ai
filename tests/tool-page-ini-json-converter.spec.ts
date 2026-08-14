import { test, expect } from './fixtures';

const CFG = `# app config
app = demo

[server]
host = localhost
port = 8080  ; listen port

[db]
name = main`;

async function runWasm(
  page: any,
  input: string,
  mode = 'auto',
  section = '',
  key = '',
  value = '',
  detectTypes = 'false',
  inlineComments = 'true',
  delimiter = 'equals_spaced',
  pretty = 'true',
  indent = '2',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/ini-json-converter/gizza_ai_ini_json_converter_web.js');
    await mod.default('/tools/ini-json-converter/gizza_ai_ini_json_converter_web_bg.wasm');
    return mod.run(
      args.input,
      args.mode,
      args.section,
      args.key,
      args.value,
      args.detectTypes,
      args.inlineComments,
      args.delimiter,
      args.pretty,
      args.indent,
    );
  }, { input, mode, section, key, value, detectTypes, inlineComments, delimiter, pretty, indent });
}

test('ini-json-converter page converts real INI to JSON', async ({ page }) => {
  await page.goto('/tools/ini-json-converter/');
  await page.fill('#in-input', CFG);
  await page.selectOption('#in-mode', 'ini_to_json');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"app": "demo"', { timeout: 20_000 });
  await expect(output).toContainText('"server":');
  await expect(output).toContainText('"port": "8080"');
  await expect(output).toContainText('"db":');
});

test('ini-json-converter deep link sets one key and preserves comments', async ({ page }) => {
  const params = new URLSearchParams({
    input: CFG,
    mode: 'set',
    key: 'server.port',
    value: '9090',
  });
  await page.goto(`/tools/ini-json-converter/?${params.toString()}`);

  await expect(page.locator('#in-mode')).toHaveValue('set', { timeout: 15_000 });
  await expect(page.locator('#in-key')).toHaveValue('server.port');
  await expect(page.locator('#in-value')).toHaveValue('9090');
  const output = page.locator('#tool-output');
  await expect(output).toContainText('port = 9090  ; listen port', { timeout: 20_000 });
  await expect(output).toContainText('# app config');
  await expect(output).toContainText('[db]');
});

test('ini-json-converter wasm covers enums, booleans, cap boundary and CLI example', async ({ page }) => {
  await page.goto('/tools/ini-json-converter/');

  const json = await runWasm(page, CFG, 'ini_to_json');
  expect(json).toContain('"port": "8080"');

  const ini = await runWasm(page, '{"server":{"host":"localhost","port":8080}}', 'json_to_ini');
  expect(ini).toBe('[server]\nhost = localhost\nport = 8080\n');

  await expect(runWasm(page, CFG, 'get', '', 'server.port')).resolves.toBe('8080');
  await expect(runWasm(page, CFG, 'set', '', 'server.port', '9090')).resolves.toContain('port = 9090  ; listen port');
  await expect(runWasm(page, CFG, 'delete', 'server', '')).resolves.not.toContain('[server]');

  await expect(runWasm(page, '{"s":{"k":"v"}}', 'json_to_ini', '', '', '', 'false', 'true', 'equals_spaced')).resolves.toBe('[s]\nk = v\n');
  await expect(runWasm(page, '{"s":{"k":"v"}}', 'json_to_ini', '', '', '', 'false', 'true', 'equals')).resolves.toBe('[s]\nk=v\n');
  await expect(runWasm(page, '{"s":{"k":"v"}}', 'json_to_ini', '', '', '', 'false', 'true', 'colon')).resolves.toBe('[s]\nk: v\n');

  const typed = await runWasm(page, '[s]\nport = 8080\ndebug = true\n', 'ini_to_json', '', '', '', 'true', 'true', 'equals_spaced', 'true', '4');
  expect(typed).toContain('\n    "s": {\n        "port": 8080,');
  expect(typed).toContain('"debug": true');

  const tabbed = await runWasm(page, '[s]\nk = v\n', 'ini_to_json', '', '', '', 'false', 'true', 'equals_spaced', 'true', 'tab');
  expect(tabbed).toContain('\n\t"s": {\n\t\t"k": "v"');

  const compactWithCommentValue = await runWasm(page, '[s]\nurl = https://x/#frag  ; keep me\n', 'ini_to_json', '', '', '', 'false', 'false', 'equals_spaced', 'false', '2');
  expect(compactWithCommentValue).toBe('{"s":{"url":"https://x/#frag  ; keep me"}}');

  const boundary = Array.from({ length: 100_000 }, (_, i) => `k${i}=v`).join('\n');
  await expect(runWasm(page, boundary, 'ini_to_json', '', '', '', 'false', 'true', 'equals_spaced', 'false', '2')).resolves.toContain('"k99999":"v"');
  await expect(runWasm(page, `${boundary}\nover=v`, 'ini_to_json')).rejects.toThrow(/100000-line limit/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool ini-json-converter');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
