import { test, expect } from './fixtures';

test('file-tree-generator paths mode (default) merges shared prefixes', async ({ page }) => {
  await page.goto('/tools/file-tree-generator/');

  await page.fill('#in-input', 'src/main.rs\nsrc/lib.rs\nREADME.md');
  // Default mode=paths, default Unicode connectors, trailing_slash on.
  await expect(page.locator('#tool-output')).toHaveText(
    '.\n├── src/\n│   ├── main.rs\n│   └── lib.rs\n└── README.md',
    { timeout: 15000 },
  );
});

test('file-tree-generator outline mode nests by indentation', async ({ page }) => {
  await page.goto('/tools/file-tree-generator/');

  await page.fill('#in-input', 'a\n  b\n  c');
  await page.selectOption('#in-mode', 'outline');
  await expect(page.locator('#tool-output')).toHaveText(
    '.\n└── a/\n    ├── b\n    └── c',
    { timeout: 15000 },
  );
});

test('file-tree-generator ascii connectors', async ({ page }) => {
  await page.goto('/tools/file-tree-generator/');

  await page.fill('#in-input', 'a/b\nc');
  await page.check('#in-ascii');
  await expect(page.locator('#tool-output')).toHaveText(
    '.\n|-- a/\n|   `-- b\n`-- c',
    { timeout: 15000 },
  );
});
