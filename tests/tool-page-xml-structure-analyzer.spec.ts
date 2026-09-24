import { test, expect } from './fixtures';

const SAMPLE_XML = `<catalog>
  <book id="b1"><title>Dune</title><price currency="USD">9.99</price></book>
  <book id="b2"><title>Emma</title><price currency="EUR">7.50</price></book>
  <meta/>
</catalog>`;

async function runWasm(
  page: any,
  xml = SAMPLE_XML,
  format = 'text',
  treeDepth = '0',
  topTags = '50',
  showAttributes = 'true',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/xml-structure-analyzer/gizza_ai_xml_structure_analyzer_web.js');
    await mod.default('/tools/xml-structure-analyzer/gizza_ai_xml_structure_analyzer_web_bg.wasm');
    return mod.run(args.xml, args.format, args.treeDepth, args.topTags, args.showAttributes);
  }, { xml, format, treeDepth, topTags, showAttributes });
}

async function setTextarea(page, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v) => {
    el.value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('xml-structure-analyzer wasm returns exact text sections and CSV', async ({ page }) => {
  await page.goto('/tools/xml-structure-analyzer/');
  await page.waitForSelector('#in-xml');

  const text = await runWasm(page);
  expect(text).toContain('Root element:  catalog');
  expect(text).toContain('Max depth:     3');
  expect(text).toContain('book (2)  [id]');
  expect(text).toContain('price (2)  [currency]');

  const csv = await runWasm(page, '<root><item a="1"/><item a="2"/></root>', 'csv');
  expect(csv).toContain('tag,count,min_depth,max_depth,max_children,text_nodes,empty,attributes');
  expect(csv).toContain('item,2,2,2,0,0,2,a=2');
});

test('xml-structure-analyzer page renders output and honors non-default checkbox', async ({ page }) => {
  await page.goto('/tools/xml-structure-analyzer/');
  await setTextarea(page, '#in-xml', '<root><item a="1"/><item a="2"/></root>');
  await page.selectOption('#in-format', 'text');
  await page.fill('#in-tree_depth', '0');
  await page.fill('#in-top_tags', '50');
  await page.uncheck('#in-show_attributes');

  await expect(page.locator('#tool-output')).toContainText('Root element:  root', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('item (2)');
  await expect(page.locator('#tool-output')).not.toContainText('[a]');
});

test('xml-structure-analyzer deep-link pre-fills and returns JSON', async ({ page }) => {
  const params = new URLSearchParams({
    xml: '<root><item a="1"/></root>',
    format: 'json',
    tree_depth: '1',
    top_tags: '1',
    show_attributes: 'false',
  });

  await page.goto(`/tools/xml-structure-analyzer/?${params.toString()}`);
  await expect(page.locator('#in-xml')).toHaveValue('<root><item a="1"/></root>', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"root": "root"', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"tree_truncated": true');
  await expect(page.locator('#tool-output')).toContainText('"tags_truncated": true');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool xml-structure-analyzer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
