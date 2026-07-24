import { test, expect } from './fixtures';

const poll = (page: any) =>
  expect.poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15000 });

test('ini-env-diff page reports added removed changed and unchanged keys', async ({ page }) => {
  await page.goto('/tools/ini-env-diff/');
  await page.fill('#in-left', '# base\nDB_HOST=localhost\nDB_PORT=5432\nDEBUG=true');
  await page.fill('#in-right', 'DB_HOST=prod.internal\nDB_PORT=5432\nNEW_FLAG=on');

  await poll(page).toBe(
    'Config diff (env) — 1 added, 1 removed, 1 changed, 1 unchanged\n\n' +
      'Added (1):\n  + NEW_FLAG = on\n\n' +
      'Removed (1):\n  - DEBUG = true\n\n' +
      'Changed (1):\n  ~ DB_HOST: localhost -> prod.internal\n\n' +
      'Unchanged (1):\n  DB_PORT'
  );
});

test('ini-env-diff query-param deep-link can force ini parsing', async ({ page }) => {
  await page.goto(
    '/tools/ini-env-diff/?left=' +
      encodeURIComponent('[db]\nhost = localhost\nport = 5432') +
      '&right=' +
      encodeURIComponent('[db]\nhost = 10.0.0.1\nport = 5432') +
      '&format=ini&output=report'
  );

  await expect(page.locator('#in-format')).toHaveValue('ini', { timeout: 15000 });
  await poll(page).toBe(
    'Config diff (ini) — 0 added, 0 removed, 1 changed, 1 unchanged\n\n' +
      'Added (0):\n  (none)\n\n' +
      'Removed (0):\n  (none)\n\n' +
      'Changed (1):\n  ~ db.host: localhost -> 10.0.0.1\n\n' +
      'Unchanged (1):\n  db.port'
  );
});

test('ini-env-diff can mask secrets and emit structured json', async ({ page }) => {
  await page.goto('/tools/ini-env-diff/');
  await page.fill('#in-left', 'API_TOKEN=dev-token-123456\nDB_HOST=localhost');
  await page.fill('#in-right', 'API_TOKEN=prod-token-987654\nDB_HOST=prod.internal');
  await page.check('#in-mask_secrets');
  await page.selectOption('#in-output', 'json');

  await expect(async () => {
    const parsed = JSON.parse((await page.locator('#tool-output').textContent()) ?? '');
    expect(parsed.summary).toEqual({ added: 0, removed: 0, changed: 2, unchanged: 0 });
    expect(parsed.changed.API_TOKEN).toEqual({ old: 'de****56', new: 'pr****54' });
    expect(parsed.changed.DB_HOST).toEqual({ old: 'localhost', new: 'prod.internal' });
  }).toPass({ timeout: 15000 });
});
