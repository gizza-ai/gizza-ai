import { test, expect } from './fixtures';

const PATHS = 'src/main.rs\nsrc/lib.rs\ntests/app.test.ts\ntarget/debug/app\nREADME.md';

test('glob-filter keeps gitignore-style matches at any depth', async ({ page }) => {
  await page.goto('/tools/glob-filter/');
  await page.fill('#in-paths', PATHS);
  await page.fill('#in-include', '*.rs');
  await page.fill('#in-exclude', 'target/');
  await page.selectOption('#in-syntax', 'gitignore');
  await page.selectOption('#in-output', 'matched');

  await expect(page.locator('#tool-output')).toHaveText('src/main.rs\nsrc/lib.rs', {
    timeout: 15000,
  });
});

test('glob-filter deep-link annotates gitignore excludes and re-includes', async ({ page }) => {
  const paths = 'src/main.rs\ntarget/debug/app\ntarget/keep.txt\nREADME.md';
  await page.goto(
    '/tools/glob-filter/?paths=' +
      encodeURIComponent(paths) +
      '&include=&exclude=' +
      encodeURIComponent('target/\n!target/keep.txt') +
      '&syntax=gitignore&case_sensitive=true&output=annotated',
  );

  await expect(page.locator('#in-paths')).toHaveValue(paths, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText(
    '✓ src/main.rs\n✗ target/debug/app\n✓ target/keep.txt\n✓ README.md',
    { timeout: 15000 },
  );
});

test('glob-filter supports whole-path glob and case-insensitive matching', async ({ page }) => {
  await page.goto('/tools/glob-filter/');
  await page.fill('#in-paths', 'README.MD\nnotes.md\nsrc/lib.rs');
  await page.fill('#in-include', '**/*.md');
  await page.selectOption('#in-syntax', 'glob');
  await page.uncheck('#in-case_sensitive');
  await page.selectOption('#in-output', 'unmatched');

  await expect(page.locator('#tool-output')).toHaveText('src/lib.rs', { timeout: 15000 });
});
