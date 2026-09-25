import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  code: string,
  preset = 'recommended',
  ecma = 'latest',
  env = 'browser',
  source_type = 'auto',
  min_severity = 'all',
  ignore = '',
  format = 'text',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/js-linter/gizza_ai_js_linter_web.js');
    await mod.default('/tools/js-linter/gizza_ai_js_linter_web_bg.wasm');
    return mod.run(
      args.code,
      args.preset,
      args.ecma,
      args.env,
      args.source_type,
      args.min_severity,
      args.ignore,
      args.format,
    );
  }, { code, preset, ecma, env, source_type, min_severity, ignore, format });
}

test('js-linter page reports real JavaScript findings', async ({ page }) => {
  await page.goto('/tools/js-linter/');
  await page.fill('#in-code', "function demo(x) {\n  var unused = 1\n  if (x == 1) console.log(x)\n  return x;\n  alert('later');\n}");
  await page.selectOption('#in-preset', 'recommended');
  await page.selectOption('#in-ecma', 'latest');
  await page.selectOption('#in-env', 'browser');
  await page.selectOption('#in-format', 'text');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('EQEQ', { timeout: 15_000 });
  await expect(out).toContainText('UNUSED-VAR');
  await expect(out).toContainText('UNREACHABLE');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool js-linter');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('js-linter deep-link prefills and filters to JSON warnings', async ({ page }) => {
  const params = new URLSearchParams({
    code: "const answer = 42\nif (answer != '42') console.log(answer)",
    preset: 'strict',
    ecma: 'es2020',
    env: 'node',
    source_type: 'script',
    min_severity: 'warning',
    ignore: 'NO-CONSOLE',
    format: 'json',
  });
  await page.goto(`/tools/js-linter/?${params.toString()}`);
  await expect(page.locator('#in-preset')).toHaveValue('strict', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"rule":"EQEQ"', { timeout: 15_000 });
  await expect(out).not.toContainText('NO-CONSOLE');
});

test('js-linter wasm covers enums, ignore list and error paths', async ({ page }) => {
  await page.goto('/tools/js-linter/');
  await page.waitForSelector('#in-code');

  const minimal = await runWasm(page, 'var a = 1\nif (a == 1) console.log(a)', 'minimal', 'es5', 'browser', 'auto', 'all', '', 'text');
  expect(minimal).toContain('EQEQ');
  expect(minimal).not.toContain('NO-VAR');

  const json = await runWasm(page, 'const a = 1', 'recommended', 'latest', 'browser', 'auto', 'all', 'UNUSED-VAR SEMICOLON', 'json');
  expect(json).toBe('{"issues":[]}');

  const script = await runWasm(page, 'import x from "y";', 'recommended', 'latest', 'node', 'script', 'error', '', 'text');
  expect(script).toContain('MODULE-SYNTAX');

  await expect(runWasm(page, '', 'recommended')).rejects.toThrow(/code is required/);
});
