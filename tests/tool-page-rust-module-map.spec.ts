import { test, expect } from './fixtures';

const SAMPLE = 'pub mod config {\n' +
  '    pub struct Config;\n' +
  '    impl Config { pub fn load() -> Self { Config } }\n' +
  '}\n' +
  'fn main() {}';

const EXPECTED_TREE =
  'crate\n' +
  '├── mod config: pub\n' +
  '│   ├── struct Config: pub\n' +
  '│   └── impl Config\n' +
  '│       └── fn load: pub\n' +
  '└── fn main: pub(self)\n';

async function fillSample(page: import('@playwright/test').Page, source = SAMPLE) {
  await page.goto('/tools/rust-module-map/');
  await page.fill('#in-source', source);
}

test('rust-module-map renders an exact tree from Rust source', async ({ page }) => {
  await fillSample(page);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('struct Config: pub', { timeout: 15_000 });
  expect(await out.textContent()).toBe(EXPECTED_TREE);
});

test('rust-module-map covers mermaid/json/paths enum outputs', async ({ page }) => {
  await fillSample(page);

  await page.selectOption('#in-format', 'mermaid');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('flowchart TD', { timeout: 15_000 });
  await expect(out).toContainText('n1["mod config: pub"]');

  await page.selectOption('#in-format', 'json');
  await expect(out).toContainText('"modules": 1', { timeout: 15_000 });
  await expect(out).toContainText('"impls": 1');
  await expect(out).toContainText('"name": "config"');

  await page.selectOption('#in-format', 'paths');
  await expect(out).toContainText('crate::config::Config::load  (fn, pub)', { timeout: 15_000 });
});

test('rust-module-map deep-links params, focus and non-default checkboxes', async ({ page }) => {
  const params = new URLSearchParams({
    source: 'pub mod config { pub struct Config; pub fn load() {} } pub trait Service {} fn main() {}',
    format: 'tree',
    max_depth: '0',
    focus_on: 'crate::config',
    sort_by: 'name',
    show_types: 'false',
    show_traits: 'true',
    show_fns: 'true',
    show_impls: 'true',
    show_consts: 'false',
    include_tests: 'false',
    show_visibility: 'false',
    crate_name: '',
  });
  await page.goto(`/tools/rust-module-map/?${params.toString()}`);

  await expect(page.locator('#in-focus_on')).toHaveValue('crate::config');
  await expect(page.locator('#in-sort_by')).toHaveValue('name');
  await expect(page.locator('#in-show_types')).not.toBeChecked();
  await expect(page.locator('#in-show_visibility')).not.toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('mod crate::config', { timeout: 15_000 });
  await expect(out).toContainText('└── fn load');
  await expect(out).not.toContainText('struct Config');
  await expect(out).not.toContainText(': pub');
  await expect(out).not.toContainText('fn main');
});

test('rust-module-map includes tests and accepts max_depth cap boundary', async ({ page }) => {
  await fillSample(page, 'pub fn parse() {}\n#[cfg(test)] mod tests { #[test] fn parse_ok() {} }');
  await page.check('#in-include_tests');
  await page.fill('#in-max_depth', '64');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('mod tests: pub(self) #[cfg(test)]', { timeout: 15_000 });
  await expect(out).toContainText('fn parse_ok: pub(self) #[test]');
});
