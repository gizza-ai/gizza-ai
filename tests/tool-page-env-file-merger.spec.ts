import { test, expect } from './fixtures';

const BASE = 'APP_NAME=demo\nAPI_URL=https://dev.example.com\nDEBUG=true\nAPI_TOKEN=dev-token-123456';
const LOCAL = 'DEBUG=false';
const PROD = 'API_URL=https://api.example.com\nCDN_URL=https://cdn.example.com';
const PROD_LOCAL = 'API_TOKEN=prod-token-987654';

async function result(page: any): Promise<string> {
  const output = page.locator('#tool-output');
  await expect(output).toContainText('.env merge report', { timeout: 15_000 });
  return (await output.textContent()) ?? '';
}

test('env-file-merger page reports winning layer and override chains', async ({ page }) => {
  await page.goto('/tools/env-file-merger/');
  await page.waitForSelector('#in-layer1');
  await page.fill('#in-layer1', BASE);
  await page.fill('#in-layer2', LOCAL);
  await page.fill('#in-layer3', PROD);
  await page.fill('#in-layer4', PROD_LOCAL);
  await page.selectOption('#in-output', 'report');
  await page.check('#in-mask_secrets');
  await page.uncheck('#in-sort_keys');
  await page.uncheck('#in-expand_vars');
  const text = await result(page);
  expect(text).toContain('Layers merged: 4 (.env < .env.local < .env.production < .env.production.local)');
  expect(text).toContain('API_URL=https://api.example.com  # set by .env.production');
  expect(text).toContain('DEBUG=false  # set by .env.local');
  expect(text).toContain('API_TOKEN=pr****54  # set by .env.production.local');
  expect(text).toContain('API_URL\n  .env');
  expect(text).toContain('DEBUG\n  .env');
});

test('env-file-merger deep link filters VITE vars and emits shell exports', async ({ page }) => {
  const params = new URLSearchParams({
    layer1: 'VITE_API_URL=https://dev.example.com\nDB_PASSWORD=hunter2000\nVITE_MODE=dev',
    layer2: 'VITE_MODE=local',
    layer3: '',
    layer4: '',
    layer_names: '.env,.env.local',
    output: 'shell',
    mask_secrets: 'false',
    sort_keys: 'true',
    prefix_filter: 'VITE_',
    expand_vars: 'false',
  });
  await page.goto(`/tools/env-file-merger/?${params.toString()}`);
  await page.waitForSelector('#in-layer1');
  await expect(page.locator('#in-output')).toHaveValue('shell', { timeout: 15_000 });
  await expect(page.locator('#in-prefix_filter')).toHaveValue('VITE_');
  const output = page.locator('#tool-output');
  await expect(output).toContainText("export VITE_API_URL='https://dev.example.com'", { timeout: 15_000 });
  const text = (await output.textContent()) ?? '';
  expect(text).toBe("export VITE_API_URL='https://dev.example.com'\nexport VITE_MODE='local'");
});
