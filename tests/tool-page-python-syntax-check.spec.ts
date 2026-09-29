import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  code: string,
  mode = 'module',
  format = 'text',
  show_context = 'true',
  python2_hints = 'true',
  stats = 'true',
  filename = '<input>',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/python-syntax-check/gizza_ai_python_syntax_check_web.js');
    await mod.default('/tools/python-syntax-check/gizza_ai_python_syntax_check_web_bg.wasm');
    return mod.run(
      args.code,
      args.mode,
      args.format,
      args.show_context,
      args.python2_hints,
      args.stats,
      args.filename,
    );
  }, { code, mode, format, show_context, python2_hints, stats, filename });
}

test('python-syntax-check page reports real parse errors', async ({ page }) => {
  await page.goto('/tools/python-syntax-check/');
  await page.fill('#in-code', 'def greet(name)\n    return name');
  await page.selectOption('#in-mode', 'module');
  await page.selectOption('#in-format', 'text');
  await page.fill('#in-filename', 'broken.py');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('broken.py:1:16: SyntaxError', { timeout: 15_000 });
  await expect(out).toContainText('def greet(name)');
  await expect(out).toContainText('^');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool python-syntax-check');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('python-syntax-check deep-link prefills and emits JSON', async ({ page }) => {
  const params = new URLSearchParams({
    code: '1 + 2 *',
    mode: 'expression',
    format: 'json',
    show_context: 'true',
    python2_hints: 'false',
    stats: 'false',
    filename: 'expr.py',
  });
  await page.goto(`/tools/python-syntax-check/?${params.toString()}`);
  await expect(page.locator('#in-mode')).toHaveValue('expression', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"filename": "expr.py"', { timeout: 15_000 });
  await expect(out).toContainText('"valid": false');
  await expect(out).toContainText('"type": "SyntaxError"');
});

test('python-syntax-check wasm covers enums, booleans and errors', async ({ page }) => {
  await page.goto('/tools/python-syntax-check/');
  await page.waitForSelector('#in-code');

  const ok = await runWasm(page, 'def f():\n    return 1\n', 'module', 'text', 'true', 'true', 'true', 'ok.py');
  expect(ok).toContain('ok.py: OK');
  expect(ok).toContain('lines: 2');

  const json = await runWasm(page, '1 + 2 *', 'expression', 'json', 'true', 'false', 'false', 'expr.py');
  expect(json).toContain('"filename": "expr.py"');
  expect(json).toContain('"valid": false');
  expect(json).not.toContain('"stats"');

  const hint = await runWasm(page, 'print "hello"\n', 'module', 'text', 'true', 'true', 'false', 'py2.py');
  expect(hint).toContain('Python 2 constructs found');

  const noContext = await runWasm(page, 'def f(:\n', 'module', 'text', 'false', 'false', 'false', 'nocaret.py');
  expect(noContext).toContain('nocaret.py:1:7: SyntaxError');
  expect(noContext).not.toContain('^');

  await expect(runWasm(page, '', 'module')).rejects.toThrow(/code is required/);
});
