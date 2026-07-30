import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('molecular-weight-calculator page — water formula output', async ({ page }) => {
  await page.goto('/tools/molecular-weight-calculator/');
  await page.fill('#in-formula', 'H2O');
  await page.fill('#in-decimals', '4');
  await expect(page.locator('#tool-output')).toContainText('"molar_mass": 18.015', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('"hill_formula": "H2O"');
  expect(out).toContain('"monoisotopic_mass": 18.0106');
  expect(out).toContain('"symbol": "H"');
  expect(out).toContain('"percent": 88.81');
});

test('molecular-weight-calculator page — grouped formula and precision', async ({ page }) => {
  await page.goto('/tools/molecular-weight-calculator/');
  await page.fill('#in-formula', 'Ca(OH)2');
  await page.fill('#in-decimals', '2');
  await expect(page.locator('#tool-output')).toContainText('"molar_mass": 74.09', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('"atom_count": 5');
  expect(out).toContain('"element_count": 3');
});

test('molecular-weight-calculator page — hydrate dot notation', async ({ page }) => {
  await page.goto('/tools/molecular-weight-calculator/');
  await page.fill('#in-formula', 'CuSO4·5H2O');
  await page.fill('#in-decimals', '3');
  await expect(page.locator('#tool-output')).toContainText('"molar_mass": 249.677', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('"hill_formula": "CuH10O9S"');
  expect(out).toContain('"atom_count": 21');
});

test('molecular-weight-calculator page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto('/tools/molecular-weight-calculator/?formula=C6H12O6&decimals=2');
  await expect(page.locator('#in-formula')).toHaveValue('C6H12O6', { timeout: 15000 });
  await expect(page.locator('#in-decimals')).toHaveValue('2');
  await expect(page.locator('#tool-output')).toContainText('"molar_mass": 180.16', { timeout: 15000 });
  expect(await outputText(page)).toContain('"hill_formula": "C6H12O6"');
});
