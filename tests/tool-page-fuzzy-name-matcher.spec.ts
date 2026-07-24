import { test, expect } from './fixtures';

const PEOPLE = `Dr. John Adams
John Adams Jr
Jon Adams
Maria Garcia`;

const ORGS = `Acme Corp
ACME Corporation
Acme Co
Globex LLC`;

test('fuzzy-name-matcher groups person name variants with a canonical mapping', async ({ page }) => {
  await page.goto('/tools/fuzzy-name-matcher/');
  await page.fill('#in-names', PEOPLE);
  await page.selectOption('#in-algorithm', 'jaro_winkler');
  await page.selectOption('#in-mode', 'groups');
  await page.fill('#in-threshold', '85');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('# Name match groups', { timeout: 15000 });
  await expect(out).toContainText('John Adams Jr');
  await expect(out).toContainText('Dr. John Adams');
  await expect(out).toContainText('Maria Garcia');
  await expect(out).toContainText('## Mapping');
});

test('fuzzy-name-matcher pairs mode returns scored organization candidates', async ({ page }) => {
  await page.goto('/tools/fuzzy-name-matcher/');
  await page.fill('#in-names', ORGS);
  await page.selectOption('#in-mode', 'pairs');
  await page.fill('#in-threshold', '80');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('# Matched name pairs', { timeout: 15000 });
  await expect(out).toContainText('Acme Corp');
  await expect(out).toContainText('ACME Corporation');
  await expect(out).toContainText('| Name A | Name B | Score |');
});

test('fuzzy-name-matcher deep-link prefills soundex/json options', async ({ page }) => {
  await page.goto('/tools/fuzzy-name-matcher/?names=Smith%0ASmyth%0ARobert%0ARupert&algorithm=soundex&mode=groups&threshold=100&normalize_case=true&ignore_titles=true&output=json');
  await expect(page.locator('#in-names')).toHaveValue('Smith\nSmyth\nRobert\nRupert', { timeout: 15000 });
  await expect(page.locator('#in-algorithm')).toHaveValue('soundex');
  await expect(page.locator('#in-mode')).toHaveValue('groups');
  await expect(page.locator('#in-threshold')).toHaveValue('100');
  await expect(page.locator('#in-output')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"algorithm": "soundex"');
  await expect(out).toContainText('"match_groups"');
});
