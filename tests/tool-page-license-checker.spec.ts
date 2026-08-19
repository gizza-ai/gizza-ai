import { test, expect } from './fixtures';

const DEPS = `chalk@4.1.2: MIT
copyleft-lib@2.0.0: GPL-3.0-only
dual@1.0.0: MIT OR Apache-2.0
mystery@0.1.0: NOASSERTION
`;

async function runWasm(
  page,
  dependencies = DEPS,
  input_format = 'list',
  allow = 'MIT, Apache-2.0, category:public-domain',
  deny = 'category:network-copyleft',
  exceptions = '',
  unlisted = 'deny',
  unknown = 'warn',
  validate_ids = 'true',
  include_allowed = 'false',
  output = 'text',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/license-checker/gizza_ai_license_checker_web.js');
    await mod.default('/tools/license-checker/gizza_ai_license_checker_web_bg.wasm');
    return mod.run(
      args.dependencies,
      args.input_format,
      args.allow,
      args.deny,
      args.exceptions,
      args.unlisted,
      args.unknown,
      args.validate_ids,
      args.include_allowed,
      args.output,
    );
  }, { dependencies, input_format, allow, deny, exceptions, unlisted, unknown, validate_ids, include_allowed, output });
}

test('license-checker wasm returns an exact policy report', async ({ page }) => {
  await page.goto('/tools/license-checker/');
  await page.waitForSelector('#in-dependencies');

  await expect(runWasm(page)).resolves.toBe(
    'License check: FAIL\n\n' +
    'Summary\n' +
    '  packages:  4\n' +
    '  allowed:   2\n' +
    '  warnings:  1\n' +
    '  denied:    1\n' +
    '  unknown:   1\n' +
    '  invalid:   0\n\n' +
    'Denied (1)\n' +
    '  x copyleft-lib@2.0.0 — GPL-3.0-only (strong-copyleft): not on the allow list\n\n' +
    'Warnings (1)\n' +
    '  ! mystery@0.1.0 — (no license) (unknown): no license metadata\n\n' +
    'Licenses in use\n' +
    '  (no license) (unknown) — 1\n' +
    '  GPL-3.0-only (strong-copyleft) — 1\n' +
    '  MIT (permissive) — 1\n' +
    '  MIT OR Apache-2.0 (permissive) — 1\n',
  );
});

test('license-checker wasm covers output enums, checkbox state, and policy choices', async ({ page }) => {
  await page.goto('/tools/license-checker/');
  await page.waitForSelector('#in-dependencies');

  const json = JSON.parse(await runWasm(page, DEPS, 'list', 'MIT', '', '', 'warn', 'allow', 'true', 'true', 'json'));
  expect(json.verdict).toBe('pass');
  expect(json.summary).toMatchObject({ packages: 4, allowed: 3, warnings: 1, denied: 0, unknown: 1, invalid: 0 });
  expect(json.allowed.map((f) => f.name)).toContain('chalk');
  expect(json.warnings[0]).toMatchObject({ name: 'copyleft-lib', reason: 'not on the allow list' });

  const markdown = await runWasm(page, DEPS, 'list', '', 'category:strong-copyleft', '', 'allow', 'warn', 'true', 'true', 'markdown');
  expect(markdown).toContain('# License check: FAIL');
  expect(markdown).toContain('| denied | copyleft-lib | 2.0.0 | GPL-3.0-only | strong-copyleft |');

  const csv = await runWasm(page, 'alpha@1.0.0: MIT\nbeta@2.0.0: Apache-2.0 WITH LLVM-exception', 'list', '', '', '', 'allow', 'warn', 'true', 'true', 'csv');
  expect(csv).toContain('allowed,alpha,1.0.0,MIT,permissive,no allow list configured');
  expect(csv).toContain('allowed,beta,2.0.0,Apache-2.0 WITH LLVM-exception,permissive,no allow list configured');

  await expect(runWasm(page, DEPS, 'list', '', '', '', 'allow', 'warn', 'true', 'false', 'xml'))
    .rejects.toThrow(/unknown output 'xml'/);
});

test('license-checker page renders form output and checkbox changes', async ({ page }) => {
  await page.goto('/tools/license-checker/');
  await page.fill('#in-dependencies', DEPS);
  await page.selectOption('#in-input_format', 'list');
  await page.fill('#in-allow', 'MIT, Apache-2.0, category:public-domain');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('License check: FAIL', { timeout: 15_000 });
  await expect(out).toContainText('copyleft-lib@2.0.0 — GPL-3.0-only');
  await expect(out).not.toContainText('+ chalk@4.1.2');

  await page.check('#in-include_allowed');
  await expect(out).toContainText('Allowed (2)', { timeout: 15_000 });
  await expect(out).toContainText('+ chalk@4.1.2 — MIT');
});

test('license-checker page pre-fills and computes from deep links', async ({ page }) => {
  const qs = new URLSearchParams({
    dependencies: DEPS,
    input_format: 'list',
    allow: 'MIT',
    unlisted: 'warn',
    unknown: 'allow',
    validate_ids: 'false',
    include_allowed: 'true',
    output: 'json',
  });
  await page.goto(`/tools/license-checker/?${qs.toString()}`);

  await expect(page.locator('#in-dependencies')).toHaveValue(DEPS, { timeout: 15_000 });
  await expect(page.locator('#in-input_format')).toHaveValue('list');
  await expect(page.locator('#in-unlisted')).toHaveValue('warn');
  await expect(page.locator('#in-unknown')).toHaveValue('allow');
  await expect(page.locator('#in-validate_ids')).not.toBeChecked();
  await expect(page.locator('#in-include_allowed')).toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"verdict": "pass"', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"name": "chalk"');
});
