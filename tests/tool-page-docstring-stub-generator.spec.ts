import { test, expect } from './fixtures';

const PY_SIG = 'def fetch(url: str, timeout: int = 30) -> dict:';
const TS_SIG = 'export async function load(id: string, opts?: LoadOptions): Promise<User> {';
const RUST_SIG = 'pub fn join(parts: &[&str], sep: &str) -> Result<String, Error> {';

async function runWasm(
  page,
  signature = PY_SIG,
  language = 'python',
  style = 'google',
  output = 'annotated',
  types = 'guess',
  placeholder = '_description_',
  raises = '',
  quote_style = 'double',
  extended_summary = 'false',
  examples = 'false',
  align_tags = 'false',
  indent_size = '4',
): Promise<string> {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/docstring-stub-generator/gizza_ai_docstring_stub_generator_web.js');
    await mod.default('/tools/docstring-stub-generator/gizza_ai_docstring_stub_generator_web_bg.wasm');
    return mod.run(
      args.signature,
      args.language,
      args.style,
      args.output,
      args.types,
      args.placeholder,
      args.raises,
      args.quote_style,
      args.extended_summary,
      args.examples,
      args.align_tags,
      args.indent_size,
    );
  }, {
    signature, language, style, output, types, placeholder, raises, quote_style,
    extended_summary, examples, align_tags, indent_size,
  });
}

test('docstring-stub-generator wasm emits exact Python Google output', async ({ page }) => {
  await page.goto('/tools/docstring-stub-generator/');
  await page.waitForSelector('#in-signature');

  await expect(runWasm(page)).resolves.toBe(
    'def fetch(url: str, timeout: int = 30) -> dict:\n' +
      '    """_description_\n\n' +
      '    Args:\n' +
      '        url (str): _description_\n' +
      '        timeout (int, optional): _description_. Defaults to 30.\n\n' +
      '    Returns:\n' +
      '        dict: _description_\n' +
      '    """',
  );
});

test('docstring-stub-generator wasm covers advertised languages and output modes', async ({ page }) => {
  await page.goto('/tools/docstring-stub-generator/');
  await page.waitForSelector('#in-signature');

  const jsdoc = await runWasm(page, TS_SIG, 'typescript', 'auto', 'docstring', 'guess', '_description_', '', 'double', 'false', 'false', 'true');
  expect(jsdoc).toContain('/**');
  expect(jsdoc).toContain('@param {string}');
  expect(jsdoc).toContain('@returns {Promise<User>} _description_');

  const rust = await runWasm(page, RUST_SIG, 'rust', 'auto', 'annotated', 'guess', '_description_', 'Error', 'double', 'false', 'true');
  expect(rust).toContain('/// # Errors');
  expect(rust).toContain('/// # Examples');
  expect(rust).toContain('pub fn join');

  const json = JSON.parse(await runWasm(page, PY_SIG, 'python', 'google', 'json'));
  expect(json.functions[0]).toMatchObject({ name: 'fetch', returns: 'dict' });
  expect(json.functions[0].params.map((p: { name: string }) => p.name)).toEqual(['url', 'timeout']);
});

test('docstring-stub-generator wasm covers Python styles, booleans and boundaries', async ({ page }) => {
  await page.goto('/tools/docstring-stub-generator/');
  await page.waitForSelector('#in-signature');

  expect(await runWasm(page, PY_SIG, 'python', 'numpy')).toContain('Parameters\n    ----------');
  expect(await runWasm(page, PY_SIG, 'python', 'sphinx')).toContain(':param url: _description_');
  expect(await runWasm(page, PY_SIG, 'python', 'epytext')).toContain('@param url: _description_');
  expect(await runWasm(page, PY_SIG, 'python', 'pep257')).toContain('Arguments:');
  expect(await runWasm(page, PY_SIG, 'python', 'google', 'docstring', 'none', 'FIXME', 'ValueError', 'single', 'true', 'true', 'false', '2'))
    .toContain("'''FIXME\n\n  FIXME\n\n  Args:");
  await expect(runWasm(page, PY_SIG, 'python', 'google', 'annotated', 'guess', '_description_', '', 'double', 'false', 'false', 'false', '9'))
    .rejects.toThrow(/indent_size/);
});

test('docstring-stub-generator page renders exact output and reacts to controls', async ({ page }) => {
  await page.goto('/tools/docstring-stub-generator/');
  await page.fill('#in-signature', PY_SIG);
  await expect(page.locator('#tool-output')).toContainText('Args:', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Returns:');

  await page.selectOption('#in-output', 'docstring');
  await page.selectOption('#in-style', 'numpy');
  await expect(page.locator('#tool-output')).toContainText('Parameters', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).not.toContainText('def fetch');
});

test('docstring-stub-generator page reports invalid input', async ({ page }) => {
  await page.goto('/tools/docstring-stub-generator/');
  await page.fill('#in-signature', 'not a signature');
  await expect(page.locator('#tool-output')).toContainText('no function signature found', { timeout: 15_000 });
});

test('docstring-stub-generator deep link prefills controls and computes JSDoc', async ({ page }) => {
  const params = new URLSearchParams({
    signature: TS_SIG,
    language: 'typescript',
    style: 'auto',
    output: 'docstring',
    types: 'guess',
    placeholder: '_description_',
    raises: '',
    quote_style: 'double',
    extended_summary: 'false',
    examples: 'false',
    align_tags: 'true',
    indent_size: '4',
  });
  await page.goto(`/tools/docstring-stub-generator/?${params.toString()}`);
  await expect(page.locator('#in-signature')).toHaveValue(TS_SIG, { timeout: 15_000 });
  await expect(page.locator('#in-language')).toHaveValue('typescript');
  await expect(page.locator('#in-align_tags')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('@returns {Promise<User>} _description_', { timeout: 15_000 });
});

test('docstring-stub-generator page ships a runnable generated CLI example', async ({ page }) => {
  await page.goto('/tools/docstring-stub-generator/');
  await page.waitForSelector('#in-signature');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool docstring-stub-generator');
  expect(cli).toContain('def fetch');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
