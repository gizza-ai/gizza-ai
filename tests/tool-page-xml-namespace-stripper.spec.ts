import { test, expect } from './fixtures';

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page: any,
  xml: string,
  mode = 'all',
  keep = '',
  conflicts = 'rename',
  removeSchemaHints = 'true',
  format = 'preserve',
  indent = '2',
  output = 'xml',
) {
  return await page.evaluate(async (args) => {
    const mod = await import(
      '/tools/xml-namespace-stripper/gizza_ai_xml_namespace_stripper_web.js'
    );
    await mod.default(
      '/tools/xml-namespace-stripper/gizza_ai_xml_namespace_stripper_web_bg.wasm',
    );
    return mod.run(
      args.xml,
      args.mode,
      args.keep,
      args.conflicts,
      args.removeSchemaHints,
      args.format,
      args.indent,
      args.output,
    );
  }, { xml, mode, keep, conflicts, removeSchemaHints, format, indent, output });
}

test('xml-namespace-stripper wasm removes declarations and prefixes exactly', async ({ page }) => {
  await page.goto('/tools/xml-namespace-stripper/');
  await page.waitForSelector('#in-xml');

  await expect(
    runWasm(
      page,
      '<a:root xmlns:a="urn:a"><a:item a:id="7">x</a:item></a:root>',
    ),
  ).resolves.toBe('<root><item id="7">x</item></root>');
});

test('xml-namespace-stripper wasm covers keep list, conflicts and report output', async ({
  page,
}) => {
  await page.goto('/tools/xml-namespace-stripper/');
  await page.waitForSelector('#in-xml');

  await expect(
    runWasm(
      page,
      '<soap:Envelope xmlns:soap="urn:s"><m:item xmlns:m="urn:m">x</m:item></soap:Envelope>',
      'all',
      'soap',
    ),
  ).resolves.toBe('<soap:Envelope xmlns:soap="urn:s"><item>x</item></soap:Envelope>');

  await expect(
    runWasm(
      page,
      '<r xmlns:a="urn:a" xmlns:b="urn:b" a:id="1" b:id="2"/>',
      'all',
      '',
      'rename',
    ),
  ).resolves.toBe('<r id="1" b_id="2"/>');

  const report = await runWasm(
    page,
    '<a:root xmlns:a="urn:a"><a:item a:id="7">x</a:item></a:root>',
    'all',
    '',
    'rename',
    'true',
    'preserve',
    '2',
    'report',
  );
  expect(report).toContain('declarations_removed,1');
  expect(report).toContain('element_prefixes_stripped,2');
  expect(report).toContain('attribute_prefixes_stripped,1');
  expect(report).toContain('a,urn:a');
});

test('xml-namespace-stripper page renders exact output and honors controls', async ({ page }) => {
  await page.goto('/tools/xml-namespace-stripper/');
  await setTextarea(page, '#in-xml', '<x:r xmlns:x="urn:x"><x:c>v</x:c></x:r>');
  await expect(page.locator('#tool-output')).toHaveText('<r><c>v</c></r>', {
    timeout: 15_000,
  });
});

test('xml-namespace-stripper deep-link prefills controls and runs exact output', async ({
  page,
}) => {
  const params = new URLSearchParams({
    xml: '<a:root xmlns:a="urn:a"><a:item a:id="7">x</a:item></a:root>',
    mode: 'all',
    keep: '',
    conflicts: 'rename',
    remove_schema_hints: 'true',
    format: 'preserve',
    indent: '2',
    output: 'xml',
  });

  await page.goto(`/tools/xml-namespace-stripper/?${params.toString()}`);
  await expect(page.locator('#in-xml')).toHaveValue(params.get('xml')!, {
    timeout: 15_000,
  });
  await expect(page.locator('#in-mode')).toHaveValue('all');
  await expect(page.locator('#tool-output')).toHaveText('<root><item id="7">x</item></root>', {
    timeout: 15_000,
  });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool xml-namespace-stripper');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
