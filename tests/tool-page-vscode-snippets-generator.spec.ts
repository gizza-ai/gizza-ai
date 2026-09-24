import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  template = 'console.log($1);',
  name = 'Console log',
  prefix = 'clog, log',
  description = 'Insert a console.log statement',
  scope = 'javascript,typescript',
  output = 'snippets-file',
  dollars = 'auto',
  indent = 'keep',
  tabSize = '2',
  finalTabstop = 'true',
  isFileTemplate = 'false',
  jsonIndent = '2',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/vscode-snippets-generator/gizza_ai_vscode_snippets_generator_web.js');
    await mod.default('/tools/vscode-snippets-generator/gizza_ai_vscode_snippets_generator_web_bg.wasm');
    return mod.run(
      args.template,
      args.name,
      args.prefix,
      args.description,
      args.scope,
      args.output,
      args.dollars,
      args.indent,
      args.tabSize,
      args.finalTabstop,
      args.isFileTemplate,
      args.jsonIndent,
    );
  }, { template, name, prefix, description, scope, output, dollars, indent, tabSize, finalTabstop, isFileTemplate, jsonIndent });
}

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('vscode-snippets-generator wasm preserves snippet syntax and output variants', async ({ page }) => {
  await page.goto('/tools/vscode-snippets-generator/');
  await page.waitForSelector('#in-template');

  const object = await runWasm(page);
  expect(object).toContain('"Console log"');
  expect(object).toContain('"prefix": [');
  expect(object).toContain('"clog"');
  expect(object).toContain('"console.log($1);$0"');
  expect(object).toContain('"scope": "javascript,typescript"');

  const entry = await runWasm(page, 'export function ${1:Component}() {\n  return <div>$0</div>;\n}', 'React function component', 'rfc', 'Create a React function component', 'typescriptreact,javascriptreact', 'entry', 'auto', 'spaces', '2', 'false', 'false', '2');
  expect(entry).toMatch(/^"React function component": \{/);
  expect(entry).toContain('${1:Component}');
  expect(entry).toContain('$0');

  const literal = await runWasm(page, 'price = $5; path = C:\\tmp', 'Literal', 'lit', '', '', 'snippets-file', 'literal');
  expect(literal).toContain('\\$5');
  expect(literal).toContain('C:');
  expect(literal).toContain('tmp');
});

test('vscode-snippets-generator page renders JSON and non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/vscode-snippets-generator/');
  await page.fill('#in-template', 'console.log($1);');
  await page.fill('#in-name', 'Console log');
  await page.fill('#in-prefix', 'clog, log');
  await page.fill('#in-description', 'Insert a console.log statement');
  await page.fill('#in-scope', 'javascript,typescript');
  await page.selectOption('#in-output', 'snippets-file');
  await page.selectOption('#in-dollars', 'auto');
  await page.selectOption('#in-indent', 'keep');
  await page.fill('#in-tab_size', '2');
  await page.check('#in-final_tabstop');
  await page.check('#in-is_file_template');
  await page.fill('#in-json_indent', '2');

  await expect(page.locator('#tool-output')).toContainText('"Console log"', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"isFileTemplate": true');
  await expect(page.locator('#tool-output')).toContainText('console.log($1);$0');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool vscode-snippets-generator');
  expect(cli).toContain('console.log');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('vscode-snippets-generator deep-link pre-fills and outputs entry shape', async ({ page }) => {
  const params = new URLSearchParams({
    template: 'export function ${1:Component}() {\n  return <div>$0</div>;\n}',
    name: 'React function component',
    prefix: 'rfc',
    description: 'Create a React function component',
    scope: 'typescriptreact,javascriptreact',
    output: 'entry',
    dollars: 'auto',
    indent: 'spaces',
    tab_size: '2',
    final_tabstop: 'false',
    is_file_template: 'false',
    json_indent: '2',
  });

  await page.goto(`/tools/vscode-snippets-generator/?${params.toString()}`);
  await expect(page.locator('#in-template')).toHaveValue('export function ${1:Component}() {\n  return <div>$0</div>;\n}', { timeout: 15_000 });
  await expect(page.locator('#in-output')).toHaveValue('entry');
  await expect(page.locator('#tool-output')).toContainText('"React function component": {', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('${1:Component}');
  expect(await outputText(page)).not.toMatch(/^\{/);
});
