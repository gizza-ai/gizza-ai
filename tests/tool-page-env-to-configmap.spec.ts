import { test, expect } from './fixtures';

test('env-to-configmap emits an exact ConfigMap manifest and quotes numeric values', async ({ page }) => {
  await page.goto('/tools/env-to-configmap/');
  await page.fill('#in-env', 'DB_HOST=localhost\nDB_PORT=5432\nLOG_LEVEL=info');
  await page.selectOption('#in-kind', 'configmap');
  await page.fill('#in-name', 'app-config');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('kind: ConfigMap', { timeout: 15000 });
  // Exact output (textContent preserves the YAML newlines/indentation).
  expect((await out.textContent())?.trim()).toBe(
    [
      'apiVersion: v1',
      'kind: ConfigMap',
      'metadata:',
      '  name: app-config',
      'data:',
      '  DB_HOST: localhost',
      '  DB_PORT: "5432"',
      '  LOG_LEVEL: info',
    ].join('\n'),
  );
});

test('env-to-configmap emits a base64 Secret with namespace and labels', async ({ page }) => {
  await page.goto('/tools/env-to-configmap/');
  await page.fill('#in-env', 'API_TOKEN=s3cr3t\nDB_PASSWORD=hunter2');
  await page.selectOption('#in-kind', 'secret');
  await page.fill('#in-name', 'app-secrets');
  await page.fill('#in-namespace', 'prod');
  await page.selectOption('#in-secret_encoding', 'data');
  await page.fill('#in-labels', 'app=web,tier=backend');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('kind: Secret', { timeout: 15000 });
  await expect(out).toContainText('namespace: prod');
  await expect(out).toContainText('labels:');
  await expect(out).toContainText('app: web');
  await expect(out).toContainText('tier: backend');
  await expect(out).toContainText('type: Opaque');
  await expect(out).toContainText('data:');
  // base64("s3cr3t") = czNjcjN0, base64("hunter2") = aHVudGVyMg==
  await expect(out).toContainText('API_TOKEN: czNjcjN0');
  await expect(out).toContainText('DB_PASSWORD: "aHVudGVyMg=="');
});

test('env-to-configmap keeps Secret values plaintext with stringData encoding', async ({ page }) => {
  await page.goto('/tools/env-to-configmap/');
  await page.fill('#in-env', 'API_TOKEN=s3cr3t');
  await page.selectOption('#in-kind', 'secret');
  await page.fill('#in-name', 'app-secrets');
  await page.selectOption('#in-secret_encoding', 'stringData');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('type: Opaque', { timeout: 15000 });
  await expect(out).toContainText('stringData:');
  await expect(out).toContainText('API_TOKEN: s3cr3t');
  // stringData carries plaintext, not the base64 form.
  await expect(out).not.toContainText('czNjcjN0');
});

test('env-to-configmap deep link prefills fields and runs', async ({ page }) => {
  const params = new URLSearchParams({
    env: 'GREETING=hello world\nPORT=8080',
    kind: 'configmap',
    name: 'greeter',
  });
  await page.goto(`/tools/env-to-configmap/?${params.toString()}`);

  await expect(page.locator('#in-name')).toHaveValue('greeter', { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('name: greeter', { timeout: 15000 });
  // A value with a space and a numeric value are both quoted to stay strings.
  await expect(out).toContainText('GREETING: "hello world"');
  await expect(out).toContainText('PORT: "8080"');
});
