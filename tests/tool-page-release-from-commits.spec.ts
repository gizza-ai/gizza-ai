import { test, expect } from './fixtures';

const FEATURE_FIX_VERSION = '1.5.0';

// /tools/release-from-commits/ computes semver bumps and release notes from pasted commits.
test('release-from-commits page computes the exact next version', async ({ page }) => {
  await page.goto('/tools/release-from-commits/');
  await page.fill('#in-current_version', '1.4.2');
  await page.fill('#in-commits', 'feat(api): add token refresh\nfix(ui): keep menu open');
  await page.selectOption('#in-output_format', 'version');

  const out = page.locator('#tool-output');
  await expect(out).toContainText(FEATURE_FIX_VERSION, { timeout: 15000 });
  expect(await out.textContent()).toBe(FEATURE_FIX_VERSION);
});

test('release-from-commits page renders grouped markdown with breaking changes', async ({ page }) => {
  await page.goto('/tools/release-from-commits/');
  await page.fill('#in-current_version', 'v1.4.2');
  await page.fill(
    '#in-commits',
    'feat(auth)!: replace session format\nfix(auth): migrate legacy cookie'
  );
  await page.fill('#in-release_date', '2026-08-29');
  await page.selectOption('#in-output_format', 'markdown');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('## v2.0.0 (2026-08-29)', { timeout: 15000 });
  await expect(out).toContainText('### Breaking Changes');
  await expect(out).toContainText('- **auth:** replace session format');
  await expect(out).toContainText('### Bug Fixes');
  await expect(out).toContainText('- **auth:** migrate legacy cookie');
});

test('release-from-commits page supports prerelease deep-linked parameters', async ({ page }) => {
  const params = new URLSearchParams({
    current_version: 'v2.0.0-rc.1',
    commits: 'fix(release): update generated notes',
    prerelease_policy: 'increment',
    output_format: 'version',
  });
  await page.goto(`/tools/release-from-commits/?${params.toString()}`);

  await expect(page.locator('#in-current_version')).toHaveValue('v2.0.0-rc.1');
  await expect(page.locator('#in-commits')).toHaveValue('fix(release): update generated notes');
  await expect(page.locator('#in-prerelease_policy')).toHaveValue('increment');
  await expect(page.locator('#in-output_format')).toHaveValue('version');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('v2.0.0-rc.2', { timeout: 15000 });
  expect(await out.textContent()).toBe('v2.0.0-rc.2');
});
