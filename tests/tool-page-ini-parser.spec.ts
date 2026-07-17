import { test, expect } from './fixtures';

const BASIC = 'app = demo\n\n[server]\nhost = localhost\nport = 8080\n\n[db]\nname = main';

test('ini-parser parses a basic INI into nested JSON', async ({ page }) => {
  await page.goto('/tools/ini-parser/');
  await page.fill('#in-ini', BASIC);
  const out = page.locator('#tool-output');
  // Global key at the root, then each [section] as a nested object; values stay
  // strings by default (detect_types off).
  await expect(out).toContainText('"app": "demo"', { timeout: 15000 });
  await expect(out).toContainText('"server"');
  await expect(out).toContainText('"host": "localhost"');
  await expect(out).toContainText('"port": "8080"');
  await expect(out).toContainText('"db"');
  await expect(out).toContainText('"name": "main"');
});

test('ini-parser report output surfaces duplicate keys and stats', async ({ page }) => {
  await page.goto('/tools/ini-parser/');
  await page.fill('#in-ini', '[db]\nhost = primary\nhost = replica\nport = 5432');
  await page.selectOption('#in-output', 'report');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"duplicates"', { timeout: 15000 });
  await expect(out).toContainText('"scope": "db"');
  await expect(out).toContainText('"key": "host"');
  await expect(out).toContainText('"count": 2');
  await expect(out).toContainText('"primary"');
  await expect(out).toContainText('"replica"');
  await expect(out).toContainText('"stats"');
});

test('ini-parser query-param deep-link prefills and outputs flat dotted keys', async ({ page }) => {
  const ini = '[server]\nhost = localhost\nport = 8080';
  await page.goto('/tools/ini-parser/?ini=' + encodeURIComponent(ini) + '&output=flat');
  await expect(page.locator('#in-ini')).toHaveValue(ini, { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('flat');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"server.host": "localhost"', { timeout: 15000 });
  await expect(out).toContainText('"server.port": "8080"');
});
