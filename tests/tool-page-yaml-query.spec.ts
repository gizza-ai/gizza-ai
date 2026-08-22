import { test, expect } from './fixtures';

// /tools/yaml-query/ runs a jq/yq-style filter over YAML in-browser.
test('yaml-query page extracts docker-compose ports as YAML', async ({ page }) => {
  await page.goto('/tools/yaml-query/');
  await page.fill('#in-yaml', 'services:\n  web:\n    image: nginx:1.27\n    ports:\n      - "80:80"\n      - "443:443"\n  db:\n    image: postgres:16');
  await page.fill('#in-query', '.services.web.ports');
  await page.selectOption('#in-input_format', 'yaml');
  await page.selectOption('#in-output_format', 'yaml');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('80:80', { timeout: 15000 });
  await expect(out).toContainText('443:443');
});

test('yaml-query page deep-link supports compact JSON output', async ({ page }) => {
  const qs =
    '?yaml=' + encodeURIComponent('services:\n  web:\n    image: nginx\n  worker:\n    image: app:latest') +
    '&query=' + encodeURIComponent('.services | keys') +
    '&input_format=yaml&output_format=json&documents=each&pretty=false&raw_output=false';
  await page.goto('/tools/yaml-query/' + qs);

  await expect(page.locator('#in-query')).toHaveValue('.services | keys', { timeout: 15000 });
  await expect(page.locator('#in-output_format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toHaveText('["web","worker"]', { timeout: 15000 });
});

test('yaml-query page slurps multi-document YAML streams', async ({ page }) => {
  await page.goto('/tools/yaml-query/');
  await page.fill('#in-yaml', 'kind: Service\nmetadata:\n  name: web\n---\nkind: Deployment\nmetadata:\n  name: api');
  await page.fill('#in-query', 'map(.metadata.name)');
  await page.selectOption('#in-documents', 'slurp');
  await page.selectOption('#in-output_format', 'json');
  await page.uncheck('#in-pretty');

  await expect(page.locator('#tool-output')).toHaveText('["web","api"]', { timeout: 15000 });
});
