import { test, expect } from './fixtures';

const tool = '/tools/pdf-structure-inspector/';
const sample = 'JVBERi0xLjEKMSAwIG9iago8PCAvVHlwZSAvQ2F0YWxvZyAvUGFnZXMgMiAwIFIgPj4KZW5kb2JqCjIgMCBvYmoKPDwgL1R5cGUgL1BhZ2VzIC9LaWRzIFszIDAgUl0gL0NvdW50IDEgPj4KZW5kb2JqCjMgMCBvYmoKPDwgL1R5cGUgL1BhZ2UgL1BhcmVudCAyIDAgUiAvTWVkaWFCb3ggWzAgMCAyMDAgMjAwXSA+PgplbmRvYmoKeHJlZgowIDQKMDAwMDAwMDAwMCA2NTUzNSBmIAowMDAwMDAwMDA5IDAwMDAwIG4gCjAwMDAwMDAwNTggMDAwMDAgbiAKMDAwMDAwMDExNSAwMDAwMCBuIAp0cmFpbGVyCjw8IC9Sb290IDEgMCBSIC9TaXplIDQgPj4Kc3RhcnR4cmVmCjE4NgolJUVPRgo=';

async function runWasm(
  page: import('@playwright/test').Page,
  params: Partial<Record<string, string>> = {},
) {
  const p = {
    input: sample,
    section: 'all',
    object_id: '',
    filter_key: '',
    max_objects: '100',
    format: 'text',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/pdf-structure-inspector/gizza_ai_pdf_structure_inspector_web.js');
    await mod.default('/tools/pdf-structure-inspector/gizza_ai_pdf_structure_inspector_web_bg.wasm');
    return mod.run(
      args.input,
      args.section,
      args.object_id,
      args.filter_key,
      args.max_objects,
      args.format,
    );
  }, p);
}

test('pdf-structure-inspector page renders a structural summary', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-input', sample);
  await page.selectOption('#in-section', 'summary');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('PDF 1.1 · 3 objects · 1 page · 329 bytes', { timeout: 20_000 });
  await expect(output).toContainText('Cross-reference   cross-reference table (/Size 4)');
  await expect(output).toContainText('/Catalog  1');
});

test('pdf-structure-inspector deep link can filter to one object', async ({ page }) => {
  const qs = new URLSearchParams({
    input: sample,
    section: 'objects',
    object_id: '1 0',
    filter_key: '',
    max_objects: '10',
    format: 'text',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(sample, { timeout: 15_000 });
  await expect(page.locator('#in-section')).toHaveValue('objects');
  await expect(page.locator('#in-object_id')).toHaveValue('1 0');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('Objects (1 shown of 1 matched)', { timeout: 20_000 });
  await expect(output).toContainText('1 0  dictionary  /Catalog');
});

test('pdf-structure-inspector wasm covers JSON, filters, caps, and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  const json = JSON.parse(await runWasm(page, { section: 'objects', filter_key: 'Page', format: 'json' }));
  expect(json.objects.matched).toBe(1);
  expect(json.objects.items.some((o: { type_name?: string }) => o.type_name === '/Page')).toBe(true);

  const capped = await runWasm(page, { section: 'objects', max_objects: '1' });
  expect(capped).toContain('Objects (1 shown of 3 matched)');
  expect(capped).toContain('more object(s) not shown');

  await expect(runWasm(page, { input: 'not base64 ***' })).rejects.toThrow(/neither valid base64/);
});
