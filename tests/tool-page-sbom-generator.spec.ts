import { test, expect } from './fixtures';

const npmLock = '{"name":"my-app","version":"1.0.0","lockfileVersion":3,"packages":{"":{"name":"my-app","version":"1.0.0"},"node_modules/lodash":{"version":"4.17.21"},"node_modules/mocha":{"version":"10.0.0","dev":true}}}';
const cargoLock = '[[package]]\nname = "demo"\nversion = "0.1.0"\n\n[[package]]\nname = "serde"\nversion = "1.0.200"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n';

async function outputText(page: any, expected: string) {
  const out = page.locator('#tool-output');
  await expect(out).toContainText(expected, { timeout: 15000 });
  const text = await out.textContent();
  expect(text).toBeTruthy();
  return text!;
}

test('sbom-generator page emits CycloneDX JSON from npm package-lock', async ({ page }) => {
  await page.goto('/tools/sbom-generator/');
  await page.fill('#in-lockfile', npmLock);
  await page.selectOption('#in-input_format', 'npm');
  await page.selectOption('#in-output', 'cyclonedx-json');
  await page.check('#in-include_dev');
  await page.check('#in-pretty');

  const text = await outputText(page, '"bomFormat": "CycloneDX"');
  const parsed = JSON.parse(text);
  expect(parsed.specVersion).toBe('1.6');
  expect(parsed.metadata.component.name).toBe('my-app');
  expect(parsed.components.some((c: any) => c.purl === 'pkg:npm/lodash@4.17.21')).toBeTruthy();
});

test('sbom-generator exercises output formats and non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/sbom-generator/');
  await page.fill('#in-lockfile', npmLock);
  await page.selectOption('#in-input_format', 'npm');
  await page.fill('#in-component_name', 'override-app');
  await page.fill('#in-component_version', '9.9.9');
  await page.uncheck('#in-include_dev');
  await page.uncheck('#in-pretty');

  await page.selectOption('#in-output', 'cyclonedx-json');
  let text = await outputText(page, 'override-app');
  expect(text).not.toContain('\n  "components"');
  expect(text).not.toContain('mocha');

  await page.selectOption('#in-output', 'spdx-json');
  text = await outputText(page, '"spdxVersion"');
  expect(JSON.parse(text).spdxVersion).toBe('SPDX-2.3');

  await page.selectOption('#in-output', 'spdx-tag');
  text = await outputText(page, 'SPDXVersion: SPDX-2.3');
  expect(text).toContain('PackageName: override-app');
});

test('sbom-generator handles cargo lock and timestamp deep-link', async ({ page }) => {
  const qs =
    '?lockfile=' + encodeURIComponent(cargoLock) +
    '&input_format=cargo&output=spdx-tag&component_name=demo' +
    '&component_version=0.1.0&include_dev=true' +
    '&timestamp=2026-07-24T12%3A00%3A00Z&pretty=true';
  await page.goto('/tools/sbom-generator/' + qs);

  await expect(page.locator('#in-input_format')).toHaveValue('cargo', { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('spdx-tag');
  await expect(page.locator('#in-component_name')).toHaveValue('demo');

  const text = await outputText(page, 'SPDXVersion: SPDX-2.3');
  expect(text).toContain('Created: 2026-07-24T12:00:00Z');
  expect(text).toContain('ExternalRef: PACKAGE-MANAGER purl pkg:cargo/serde@1.0.200');
});
