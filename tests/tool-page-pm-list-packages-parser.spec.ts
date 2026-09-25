import { test, expect } from './fixtures';

const tool = '/tools/pm-list-packages-parser/';
const sample = [
  'package:/data/app/~~kJ2vQ==/com.example.notes-9QeL==/base.apk=com.example.notes',
  'package:/data/app/~~7bR1w==/com.android.chrome-Ax2T==/base.apk=com.android.chrome',
  'package:/system/priv-app/Settings/Settings.apk=com.android.settings',
  'package:/vendor/app/CarrierHelper/CarrierHelper.apk=com.carrier.helper',
].join('\n');

async function runWasm(
  page: import('@playwright/test').Page,
  params: Partial<Record<string, string>> = {},
) {
  const p = {
    input: sample,
    filter: 'all',
    format: 'table',
    sort: 'package',
    group: 'true',
    disabled_list: '',
    system_list: '',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/pm-list-packages-parser/gizza_ai_pm_list_packages_parser_web.js');
    await mod.default('/tools/pm-list-packages-parser/gizza_ai_pm_list_packages_parser_web_bg.wasm');
    return mod.run(
      args.input,
      args.filter,
      args.format,
      args.sort,
      args.group,
      args.disabled_list,
      args.system_list,
    );
  }, p);
}

test('pm-list-packages-parser page renders a package classification table', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-input', sample);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('4 packages: 2 user, 2 system, 0 updated system', {
    timeout: 20_000,
  });
  await expect(output).toContainText('com.example.notes');
  await expect(output).toContainText('com.android.settings');
  await expect(output).toContainText('/system/priv-app');
});

test('pm-list-packages-parser deep link can prefill filtered CSV output', async ({ page }) => {
  const qs = new URLSearchParams({ input: sample, filter: 'user', format: 'csv', sort: 'path', group: 'false' });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(sample, { timeout: 15_000 });
  await expect(page.locator('#in-filter')).toHaveValue('user');
  await expect(page.locator('#in-format')).toHaveValue('csv');
  await expect(page.locator('#in-sort')).toHaveValue('path');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('package,type,status,partition,apk_path,installer,version_code,uid', {
    timeout: 20_000,
  });
  await expect(output).toContainText('com.android.chrome,user,unknown,/data/app');
  await expect(output).not.toContainText('com.android.settings,system');
});

test('pm-list-packages-parser wasm covers formats, filters, optional lists, and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  expect(await runWasm(page, { format: 'list', filter: 'system' })).toBe(
    'com.android.settings\ncom.carrier.helper',
  );
  expect(await runWasm(page, { format: 'csv', filter: 'user', group: 'false' })).toContain(
    'com.android.chrome,user,unknown,/data/app',
  );
  const json = JSON.parse(await runWasm(page, { format: 'json' }));
  expect(json.total).toBe(4);
  expect(json.packages.some((row: { package: string; type: string }) => row.package === 'com.example.notes' && row.type === 'user')).toBe(true);

  const withLists = await runWasm(page, {
    disabled_list: 'package:com.carrier.helper',
    system_list: 'package:com.android.chrome\npackage:com.carrier.helper',
    filter: 'disabled',
    group: 'false',
  });
  expect(withLists).toContain('1 of 4 packages shown (filter: disabled)');
  expect(withLists).toContain('com.carrier.helper');
  expect(withLists).toContain('disabled');

  await expect(runWasm(page, { input: 'package:/data/app/base.apk=' })).rejects.toThrow(
    /could not read a package name/,
  );
});
