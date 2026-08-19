import { test, expect } from './fixtures';

const sampleYaml = `server:
  host: localhost   # bind address
  port: 8080
items:
  - name: alpha
    qty: 1
  - name: beta
    qty: 2
`;

test('yaml-path-query page queries a scalar by dotted path', async ({ page }) => {
  await page.goto('/tools/yaml-path-query/');
  await page.fill('#in-yaml', sampleYaml);
  await page.fill('#in-path', 'server.host');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('localhost', { timeout: 15_000 });
  expect(await out.textContent()).toBe('localhost');
});

test('yaml-path-query page deep-link sets a value and preserves comments', async ({ page }) => {
  const qs =
    '?yaml=' + encodeURIComponent(sampleYaml) +
    '&path=' + encodeURIComponent('server.port') +
    '&mode=set' +
    '&value=9090' +
    '&format=yaml';
  await page.goto('/tools/yaml-path-query/' + qs);

  await expect(page.locator('#in-path')).toHaveValue('server.port', { timeout: 15_000 });
  await expect(page.locator('#in-mode')).toHaveValue('set');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('port: 9090', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`server:
  host: localhost   # bind address
  port: 9090
items:
  - name: alpha
    qty: 1
  - name: beta
    qty: 2
`);
});

test('yaml-path-query page returns bracket-index query as JSON', async ({ page }) => {
  await page.goto('/tools/yaml-path-query/');
  await page.fill('#in-yaml', sampleYaml);
  await page.fill('#in-path', 'items[1]');
  await page.selectOption('#in-format', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"name": "beta"', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`{
  "name": "beta",
  "qty": 2
}`);
});

test('yaml-path-query page supports quoted keys with dots', async ({ page }) => {
  await page.goto('/tools/yaml-path-query/');
  await page.fill('#in-yaml', '"my.key":\n  answer: 42\n');
  await page.fill('#in-path', '["my.key"].answer');

  await expect(page.locator('#tool-output')).toHaveText('42', { timeout: 15_000 });
});
