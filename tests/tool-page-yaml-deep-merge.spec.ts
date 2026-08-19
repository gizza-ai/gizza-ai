import { test, expect } from './fixtures';

const HELM_VALUES = `image:
  repository: app
  tag: "1.0"
replicas: 1
service:
  ports:
    - 80
---
image:
  tag: "2.0"
service:
  ports:
    - 443
`;

test('yaml-deep-merge page deep-merges Helm-style values', async ({ page }) => {
  await page.goto('/tools/yaml-deep-merge/');
  await page.fill('#in-documents', HELM_VALUES);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('repository: app', { timeout: 15000 });
  await expect(out).toContainText('tag: "2.0"');
  await expect(out).toContainText('- 443');
  await expect(out).not.toContainText('- 80');
});

test('yaml-deep-merge page deep-link appends unique arrays and sorts keys', async ({ page }) => {
  const params = new URLSearchParams({
    documents: `tags:\n  - api\n  - stable\n---\ntags:\n  - stable\n  - canary\nz: 1\n---\na: 2\n`,
    precedence: 'last',
    object_merge: 'deep',
    array_merge: 'unique',
    array_key: 'name',
    null_deletes: 'true',
    sort_keys: 'true',
    indent: '2',
  });
  await page.goto(`/tools/yaml-deep-merge/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('a: 2', { timeout: 15000 });
  await expect(out).toContainText('tags:');
  await expect(out).toContainText('- api');
  await expect(out).toContainText('- stable');
  await expect(out).toContainText('- canary');
});

test('yaml-deep-merge page reports conflict paths', async ({ page }) => {
  const params = new URLSearchParams({
    documents: 'image:\n  tag: 1.0\n---\nimage:\n  tag: 2.0\n',
    precedence: 'error',
    object_merge: 'deep',
    array_merge: 'replace',
    array_key: 'name',
    null_deletes: 'true',
    sort_keys: 'false',
    indent: '2',
  });
  await page.goto(`/tools/yaml-deep-merge/?${params.toString()}`);
  await expect(page.locator('#tool-output')).toContainText('conflict at `image.tag`', { timeout: 15000 });
});
