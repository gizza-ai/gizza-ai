import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  xml = '<order id="7"><total>19.95</total><item sku="PEN">pen</item><item sku="PAD">pad</item></order>',
  design = 'venetian-blind',
  typeInference = 'smart',
  occurrence = 'restricted',
  enumerations = '0',
  targetNamespace = '',
  indent = '2',
  declaration = 'true',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/xsd-from-xml/gizza_ai_xsd_from_xml_web.js');
    await mod.default('/tools/xsd-from-xml/gizza_ai_xsd_from_xml_web_bg.wasm');
    return mod.run(
      args.xml,
      args.design,
      args.typeInference,
      args.occurrence,
      args.enumerations,
      args.targetNamespace,
      args.indent,
      args.declaration,
    );
  }, { xml, design, typeInference, occurrence, enumerations, targetNamespace, indent, declaration });
}

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('xsd-from-xml wasm infers structure, types, enums, namespaces and design variants', async ({ page }) => {
  await page.goto('/tools/xsd-from-xml/');
  await page.waitForSelector('#in-xml');

  const base = await runWasm(page);
  expect(base).toContain('<xs:element name="order" type="orderType"/>');
  expect(base).toContain('<xs:element name="total" type="xs:decimal"/>');
  expect(base).toContain('<xs:attribute name="id" type="xs:int" use="required"/>');
  expect(base).toContain('maxOccurs="unbounded"');

  const loose = await runWasm(page, '<order><status>new</status><status>paid</status></order>', 'salami-slice', 'string', 'relaxed', '3', 'https://example.com/orders', '0', 'false');
  expect(loose).not.toContain('<?xml');
  expect(loose).toContain('targetNamespace="https://example.com/orders"');
  expect(loose).toContain('<xs:element ref="tns:status" minOccurs="0" maxOccurs="unbounded"/>');
  expect(loose).toContain('<xs:enumeration value="new"/>');

  const doll = await runWasm(page, '<root><child>42</child></root>', 'russian-doll');
  expect(doll).toContain('<xs:element name="root">');
  expect(doll).not.toContain('<xs:complexType name=');
});

test('xsd-from-xml page renders real XSD and non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/xsd-from-xml/');
  await page.fill('#in-xml', '<order id="7"><total>19.95</total><item sku="PEN">pen</item><item sku="PAD">pad</item></order>');
  await page.selectOption('#in-design', 'venetian-blind');
  await page.selectOption('#in-type_inference', 'smart');
  await page.selectOption('#in-occurrence', 'restricted');
  await page.fill('#in-enumerations', '0');
  await page.fill('#in-target_namespace', '');
  await page.fill('#in-indent', '2');
  await page.uncheck('#in-declaration');

  await expect(page.locator('#tool-output')).toContainText('<xs:schema xmlns:xs=', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('<xs:element name="item" type="itemType" maxOccurs="unbounded"/>');
  await expect(page.locator('#tool-output')).not.toContainText('<?xml');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool xsd-from-xml');
  expect(cli).toContain('<order');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('xsd-from-xml deep-link pre-fills and outputs namespaced schema', async ({ page }) => {
  const params = new URLSearchParams({
    xml: '<order xmlns="urn:orders"><id>1001</id><paid>true</paid></order>',
    design: 'venetian-blind',
    type_inference: 'smart',
    occurrence: 'restricted',
    enumerations: '0',
    target_namespace: 'https://example.com/orders',
    indent: '2',
    declaration: 'true',
  });

  await page.goto(`/tools/xsd-from-xml/?${params.toString()}`);
  await expect(page.locator('#in-xml')).toHaveValue('<order xmlns="urn:orders"><id>1001</id><paid>true</paid></order>', { timeout: 15_000 });
  await expect(page.locator('#in-target_namespace')).toHaveValue('https://example.com/orders');
  await expect(page.locator('#tool-output')).toContainText('targetNamespace="https://example.com/orders"', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('<xs:element name="paid" type="xs:boolean"/>');
  expect(await outputText(page)).toContain('elementFormDefault="qualified"');
});
