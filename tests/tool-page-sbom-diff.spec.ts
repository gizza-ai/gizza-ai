import { test, expect } from './fixtures';

const oldNpm = '{"name":"my-app","version":"1.0.0","lockfileVersion":3,"packages":{"":{"name":"my-app","version":"1.0.0"},"node_modules/chalk":{"version":"4.1.2"},"node_modules/left-pad":{"version":"1.3.0"}}}';
const newNpm = '{"name":"my-app","version":"1.1.0","lockfileVersion":3,"packages":{"":{"name":"my-app","version":"1.1.0"},"node_modules/chalk":{"version":"5.0.0"},"node_modules/lodash":{"version":"4.17.21"}}}';
const oldCargo = '[[package]]\nname = "demo"\nversion = "0.1.0"\n\n[[package]]\nname = "serde"\nversion = "1.0.200"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n';
const newCargo = '[[package]]\nname = "demo"\nversion = "0.1.0"\n\n[[package]]\nname = "serde"\nversion = "1.0.210"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n\n[[package]]\nname = "anyhow"\nversion = "1.0.86"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n';

async function outputText(page: any, expected: string) {
  const out = page.locator('#tool-output');
  await expect(out).toContainText(expected, { timeout: 15000 });
  const text = await out.textContent();
  expect(text).toBeTruthy();
  return text!;
}

test('sbom-diff page reports added, removed, and version-bumped npm deps', async ({ page }) => {
  await page.goto('/tools/sbom-diff/');
  await page.fill('#in-old', oldNpm);
  await page.fill('#in-new', newNpm);
  await page.selectOption('#in-old_format', 'npm');
  await page.selectOption('#in-new_format', 'npm');
  await page.check('#in-include_dev');
  await page.selectOption('#in-output', 'text');

  const text = await outputText(page, 'Dependency diff');
  expect(text).toContain('+ npm lodash@4.17.21');
  expect(text).toContain('- npm left-pad@1.3.0');
  expect(text).toContain('~ npm chalk 4.1.2 -> 5.0.0  (upgraded)');
});

test('sbom-diff page emits markdown and json outputs', async ({ page }) => {
  await page.goto('/tools/sbom-diff/');
  await page.fill('#in-old', oldNpm);
  await page.fill('#in-new', newNpm);
  await page.selectOption('#in-old_format', 'npm');
  await page.selectOption('#in-new_format', 'npm');

  await page.selectOption('#in-output', 'markdown');
  let text = await outputText(page, '| Change | Ecosystem | Package | Old | New |');
  expect(text).toContain('| added | npm | lodash |  | 4.17.21 |');
  expect(text).toContain('| upgraded | npm | chalk | 4.1.2 | 5.0.0 |');

  await page.selectOption('#in-output', 'json');
  text = await outputText(page, '"summary"');
  const parsed = JSON.parse(text);
  expect(parsed.summary).toEqual({ added: 1, removed: 1, changed: 1, unchanged: 0 });
  expect(parsed.changed[0].name).toBe('chalk');
  expect(parsed.changed[0].direction).toBe('upgraded');
});

test('sbom-diff handles a cargo add/remove deep-link', async ({ page }) => {
  const qs =
    '?old=' + encodeURIComponent(oldCargo) +
    '&new=' + encodeURIComponent(newCargo) +
    '&old_format=cargo&new_format=cargo&include_dev=true&output=text';
  await page.goto('/tools/sbom-diff/' + qs);

  await expect(page.locator('#in-old_format')).toHaveValue('cargo', { timeout: 15000 });
  await expect(page.locator('#in-new_format')).toHaveValue('cargo');
  await expect(page.locator('#in-output')).toHaveValue('text');

  const text = await outputText(page, 'Dependency diff');
  expect(text).toContain('+ cargo anyhow@1.0.86');
  expect(text).toContain('~ cargo serde 1.0.200 -> 1.0.210  (upgraded)');
});
