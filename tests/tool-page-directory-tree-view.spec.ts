import { test, expect } from './fixtures';

const LISTING = '1024\tsrc/main.rs\n3072\tsrc/lib.rs\n512\tREADME.md\n4096\tdocs/guide.md';

async function setListing(page: import('@playwright/test').Page, listing = LISTING) {
  await page.locator('#in-input').evaluate((el, value) => {
    (el as HTMLTextAreaElement).value = value;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, listing);
}

async function treeOutput(page: import('@playwright/test').Page) {
  const out = page.locator('#tool-output');
  await expect(out).toContainText('8.5K total · 2 directories · 4 files', { timeout: 15_000 });
  return out;
}

test('directory-tree-view renders a size-annotated tree from du-style input', async ({ page }) => {
  await page.goto('/tools/directory-tree-view/');
  await setListing(page);

  const out = await treeOutput(page);
  await expect(out).toContainText('.  8.5K  (4 files, 2 dirs)');
  await expect(out).toContainText('├── docs/  4.0K  (1 files, 0 dirs)');
  await expect(out).toContainText('│   └── guide.md  4.0K');
  await expect(out).toContainText('├── src/  4.0K  (2 files, 0 dirs)');
  await expect(out).toContainText('└── README.md  512B');
});

test('directory-tree-view deep-links CSV input with size-desc sorting', async ({ page }) => {
  const qs = new URLSearchParams({
    input: 'src/main.rs,1024\nsrc/lib.rs,3072\nREADME.md,512\ndocs/guide.md,4096',
    format: 'path-first',
    sort: 'size-desc',
    root: 'project',
  });
  await page.goto(`/tools/directory-tree-view/?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(/src\/main\.rs,1024/);
  await expect(page.locator('#in-format')).toHaveValue('path-first');
  await expect(page.locator('#in-sort')).toHaveValue('size-desc');
  await expect(page.locator('#in-root')).toHaveValue('project');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('project  8.5K  (4 files, 2 dirs)', { timeout: 15_000 });
  await expect(out).toContainText('├── docs/  4.0K  (1 files, 0 dirs)');
  await expect(out).toContainText('├── src/  4.0K  (2 files, 0 dirs)');
});

test('directory-tree-view covers enums, non-default checkbox states, and depth boundary', async ({ page }) => {
  await page.goto('/tools/directory-tree-view/');
  await setListing(page);
  await page.selectOption('#in-units', 'bytes');
  await page.selectOption('#in-format', 'size-first');
  await page.selectOption('#in-sort', 'input');
  await page.uncheck('#in-dirs_first');
  await page.uncheck('#in-trailing_slash');
  await page.uncheck('#in-show_counts');
  await page.fill('#in-depth', '1');
  await page.check('#in-ascii');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('.  8,704', { timeout: 15_000 });
  await expect(out).toContainText('|-- src  4,096');
  await expect(out).toContainText('|-- README.md  512');
  await expect(out).toContainText('`-- docs  4,096');
  await expect(out).not.toContainText('main.rs');
  await expect(out).not.toContainText('(4 files, 2 dirs)');
  await expect(out).toContainText('8,704 total · 2 directories · 4 files');
});

test('directory-tree-view reports useful validation errors', async ({ page }) => {
  await page.goto('/tools/directory-tree-view/');
  await setListing(page, 'not-a-size\tREADME.md');
  await page.selectOption('#in-format', 'size-first');
  await expect(page.locator('#tool-output')).toContainText('line 1', { timeout: 15_000 });

  await setListing(page, '');
  await expect(page.locator('#tool-output')).toContainText('no entries', { timeout: 15_000 });
});
