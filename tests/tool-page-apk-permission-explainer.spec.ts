import { test, expect } from './fixtures';

const tool = '/tools/apk-permission-explainer/';
const sample = '<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.demo"><uses-sdk android:minSdkVersion="24" android:targetSdkVersion="34"/><uses-permission android:name="android.permission.CAMERA"/><uses-permission android:name="android.permission.INTERNET"/><uses-permission android:name="com.google.android.gms.permission.AD_ID"/></manifest>';

async function runWasm(
  page: import('@playwright/test').Page,
  params: Partial<Record<string, string>> = {},
) {
  const p = {
    input: sample,
    mode: 'report',
    risk: 'all',
    sort: 'risk',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/apk-permission-explainer/gizza_ai_apk_permission_explainer_web.js');
    await mod.default('/tools/apk-permission-explainer/gizza_ai_apk_permission_explainer_web_bg.wasm');
    return mod.run(args.input, args.mode, args.risk, args.sort);
  }, p);
}

test('apk-permission-explainer page renders a permission risk report', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-input', sample);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Permissions requested by com.example.demo', { timeout: 20_000 });
  await expect(output).toContainText('CAMERA');
  await expect(output).toContainText('Google Advertising ID');
  await expect(output).toContainText('INTERNET');
});

test('apk-permission-explainer deep link can prefill risky list output', async ({ page }) => {
  const qs = new URLSearchParams({ input: sample, mode: 'list', risk: 'risky', sort: 'name' });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(sample, { timeout: 15_000 });
  await expect(page.locator('#in-mode')).toHaveValue('list');
  await expect(page.locator('#in-risk')).toHaveValue('risky');
  await expect(page.locator('#in-sort')).toHaveValue('name');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('[dangerous] android.permission.CAMERA', { timeout: 20_000 });
  await expect(output).toContainText('[privacy-sensitive] com.google.android.gms.permission.AD_ID');
  await expect(output).not.toContainText('android.permission.INTERNET');
});

test('apk-permission-explainer wasm covers formats, filters, sorting, and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  expect(await runWasm(page, { mode: 'list', risk: 'risky', sort: 'name' })).toContain(
    'com.google.android.gms.permission.AD_ID',
  );
  expect(await runWasm(page, { mode: 'list', risk: 'risky', sort: 'name' })).not.toContain(
    'android.permission.INTERNET',
  );
  expect(await runWasm(page, { mode: 'csv', risk: 'dangerous', sort: 'name' })).toContain(
    'android.permission.CAMERA,dangerous,Take photos',
  );
  const json = JSON.parse(await runWasm(page, { mode: 'json' }));
  expect(json.package).toBe('com.example.demo');
  expect(json.summary.dangerous).toBe(1);
  expect(json.summary['privacy-sensitive']).toBe(1);

  await expect(runWasm(page, { input: 'not base64 and not xml' })).rejects.toThrow(/could not decode/);
});
