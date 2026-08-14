import { test, expect } from './fixtures';

const LUA = `--! @license MIT
local function greet(name)
  -- build a greeting
  local message = "hello, " .. name
  print(message)
end

greet("Ada")`;

async function runWasm(
  page: any,
  code = LUA,
  removeComments = 'true',
  keepLicense = 'true',
  renameLocals = 'false',
  lineBreaks = 'strip',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/lua-minifier/gizza_ai_lua_minifier_web.js');
    await mod.default('/tools/lua-minifier/gizza_ai_lua_minifier_web_bg.wasm');
    return mod.run(
      args.code,
      args.removeComments,
      args.keepLicense,
      args.renameLocals,
      args.lineBreaks,
    );
  }, { code, removeComments, keepLicense, renameLocals, lineBreaks });
}

test('lua-minifier page strips comments and preserves license banners', async ({ page }) => {
  await page.goto('/tools/lua-minifier/');
  await page.fill('#in-code', LUA);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('--! @license MIT', { timeout: 20_000 });
  await expect(output).toContainText('local function greet(name)local message="hello, "..name print(message)end greet("Ada")');
  await expect(output).not.toContainText('build a greeting');
});

test('lua-minifier deep link covers local renaming and kept line breaks', async ({ page }) => {
  const params = new URLSearchParams({
    code: 'local total = 0\nfor index = 1, 3 do\n  total = total + index\nend\nreturn total',
    remove_comments: 'true',
    keep_license: 'true',
    rename_locals: 'true',
    line_breaks: 'keep',
  });
  await page.goto(`/tools/lua-minifier/?${params.toString()}`);

  await expect(page.locator('#in-rename_locals')).toBeChecked({ timeout: 15_000 });
  await expect(page.locator('#in-line_breaks')).toHaveValue('keep');
  await expect(page.locator('#tool-output')).toContainText('local a=0\nfor b=1,3 do\na=a+b\nend\nreturn a', { timeout: 20_000 });
});

test('lua-minifier wasm covers options and exact errors', async ({ page }) => {
  await page.goto('/tools/lua-minifier/');

  expect(await runWasm(page, '-- note\nlocal x = 1\nprint( x )')).toBe('local x=1 print(x)');
  expect(await runWasm(page, '--! @license MIT\n-- drop\nreturn 1', 'true', 'true')).toBe('--! @license MIT\nreturn 1');
  expect(await runWasm(page, '--! @license MIT\nreturn 1', 'true', 'false')).toBe('return 1');
  expect(await runWasm(page, 'local total = 0\nreturn total', 'true', 'true', 'true')).toBe('local a=0 return a');
  expect(await runWasm(page, 'local a = 1 -- note\nlocal b = 2', 'false', 'true', 'false', 'keep')).toBe('local a=1-- note\nlocal b=2');
  await expect(runWasm(page, '')).rejects.toThrow(/no Lua input/);
});

test('lua-minifier ships a clean runnable CLI example', async ({ page }) => {
  await page.goto('/tools/lua-minifier/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool lua-minifier');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
