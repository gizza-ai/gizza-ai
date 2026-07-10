import { test, expect } from './fixtures';

const env = '# database\nexport DB_HOST=localhost\nDB_PORT=5432\nAPI_TOKEN=secret123456\nDB_HOST=127.0.0.1';

test('dotenv-manager page report masks secrets and flags duplicates', async ({ page }) => {
  await page.goto('/tools/dotenv-manager/');
  await page.fill('#in-env', env);
  await page.fill('#in-required_keys', 'DATABASE_URL,API_TOKEN');
  await page.selectOption('#in-output', 'report');

  await expect(async () => {
    const out = await page.locator('#tool-output').textContent();
    expect(out).toContain('Duplicate keys: 1');
    expect(out).toContain('DB_HOST (lines 2, 5)');
    expect(out).toContain('Missing required keys:\n  DATABASE_URL');
    expect(out).toContain('API_TOKEN=se****56');
    expect(out).toContain('DB_HOST=127.0.0.1');
  }).toPass({ timeout: 15000 });
});

test('dotenv-manager query-param deep-link can output sorted .env.example', async ({ page }) => {
  await page.goto(
    '/tools/dotenv-manager/?env=' + encodeURIComponent('B_KEY=2\nA_KEY=1') +
      '&sort_keys=true&output=example'
  );

  await expect(page.locator('#in-output')).toHaveValue('example', { timeout: 15000 });
  await expect(async () => {
    const out = await page.locator('#tool-output').textContent();
    expect(out).toBe('A_KEY=\nB_KEY=');
  }).toPass({ timeout: 15000 });
});

test('dotenv-manager page can disable masking for normalized overlay output', async ({ page }) => {
  await page.goto('/tools/dotenv-manager/');
  await page.fill('#in-env', 'DB_HOST=localhost\nAPI_TOKEN=dev-token\nDEBUG=true');
  await page.fill('#in-merge', 'DB_HOST=prod.internal\nAPI_TOKEN=prod-token-9999');
  await page.uncheck('#in-mask_secrets');
  await page.selectOption('#in-output', 'normalized');

  await expect(async () => {
    const out = await page.locator('#tool-output').textContent();
    expect(out).toBe('DB_HOST=prod.internal\nAPI_TOKEN=prod-token-9999\nDEBUG=true');
  }).toPass({ timeout: 15000 });
});
