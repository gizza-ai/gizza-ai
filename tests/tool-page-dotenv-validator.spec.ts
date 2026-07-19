import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

// A messy .env with an unquoted space (GREETING), an undefined interpolation
// reference (${MISSING}) and a duplicate key (DB_HOST) — four distinct keys.
const MESSY = 'DB_HOST=localhost\nDB_PORT=5432\nGREETING=hello world\nURL=http://${DB_HOST}:${DB_PORT}/${MISSING}\nDB_HOST=127.0.0.1';

test('dotenv-validator reports duplicate keys, unquoted spaces and undefined refs by default', async ({ page }) => {
  await page.goto('/tools/dotenv-validator/');
  // Both rule-group toggles default ON (descriptor default true).
  await expect(page.locator('#in-check_interpolation')).toBeChecked();
  await expect(page.locator('#in-require_quotes_for_spaces')).toBeChecked();

  await page.fill('#in-env', MESSY);
  await expect(page.locator('#tool-output')).toContainText('.env validation', { timeout: 15000 });

  const out = await output(page);
  // Header tallies the real result: 3 warnings, no errors, across 4 keys.
  expect(out).toContain('3 issues (0 errors, 3 warnings) across 4 keys');
  // Each rule fires with its real message.
  expect(out).toContain('unquoted-space');
  expect(out).toContain('undefined-reference');
  expect(out).toContain('${MISSING}');
  expect(out).toContain('duplicate-key');
  expect(out).toContain('was already defined on line 1');
  // Defined references resolve and are NOT flagged.
  expect(out).not.toContain('${DB_HOST}');
});

test('dotenv-validator json output enum produces a structured CI object', async ({ page }) => {
  await page.goto('/tools/dotenv-validator/');
  await page.fill('#in-env', MESSY);
  await page.selectOption('#in-output', 'json');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"issues"', { timeout: 15000 });

  const out = await output(page);
  const parsed = JSON.parse(out);
  expect(parsed.ok).toBe(true); // warnings only, no hard errors
  expect(parsed.keys).toBe(4);
  expect(parsed.error_count).toBe(0);
  expect(parsed.warning_count).toBe(3);
  const rules = parsed.issues.map((i: any) => i.rule);
  expect(rules).toContain('duplicate-key');
  expect(rules).toContain('unquoted-space');
  expect(rules).toContain('undefined-reference');
});

test('dotenv-validator honors a non-default checkbox (require_quotes_for_spaces off)', async ({ page }) => {
  await page.goto('/tools/dotenv-validator/');
  await page.fill('#in-env', MESSY);
  await page.uncheck('#in-require_quotes_for_spaces');
  await expect(page.locator('#in-require_quotes_for_spaces')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('.env validation', { timeout: 15000 });

  const out = await output(page);
  // With the space rule off, GREETING's unquoted space is no longer reported…
  expect(out).not.toContain('unquoted-space');
  // …but the interpolation and duplicate rules still fire: 2 issues remain.
  expect(out).toContain('2 issues (0 errors, 2 warnings) across 4 keys');
  expect(out).toContain('undefined-reference');
  expect(out).toContain('duplicate-key');
});

test('dotenv-validator deep-link pre-fills params and runs on load', async ({ page }) => {
  const params = new URLSearchParams({
    env: MESSY,
    output: 'json',
    check_interpolation: 'false', // non-default: silence the interpolation rule group
  });
  await page.goto(`/tools/dotenv-validator/?${params.toString()}`);

  await expect(page.locator('#in-env')).toHaveValue(MESSY, { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-check_interpolation')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('"issues"', { timeout: 15000 });

  const parsed = JSON.parse(await output(page));
  expect(parsed.ok).toBe(true);
  expect(parsed.keys).toBe(4);
  const rules = parsed.issues.map((i: any) => i.rule);
  // Interpolation checking is off, so the undefined reference is not reported…
  expect(rules).not.toContain('undefined-reference');
  // …while the unquoted-space and duplicate-key rules still fire.
  expect(rules).toContain('unquoted-space');
  expect(rules).toContain('duplicate-key');
});
