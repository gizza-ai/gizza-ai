import { test, expect } from './fixtures';

const SIMPLE_XSD = '<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"><xs:element name="id" type="xs:string"/></xs:schema>';
const ORDER_XSD = '<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"><xs:element name="order"><xs:complexType><xs:sequence><xs:element name="total" type="xs:decimal"/></xs:sequence><xs:attribute name="currency" type="xs:string" use="required"/></xs:complexType></xs:element></xs:schema>';

test('xsd-to-json-schema page converts a simple XSD to Draft 2020-12 JSON Schema', async ({ page }) => {
  await page.goto('/tools/xsd-to-json-schema/');
  await page.fill('#in-xsd', SIMPLE_XSD);
  await expect(page.locator('#in-draft')).toHaveValue('2020-12');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"$schema": "https://json-schema.org/draft/2020-12/schema"', { timeout: 15_000 });
  await expect(out).toContainText('"title": "id"');
  await expect(out).toContainText('"type": "string"');
});

test('xsd-to-json-schema deep-link emits Draft-07 and custom attribute prefix', async ({ page }) => {
  const qs = new URLSearchParams({
    xsd: ORDER_XSD,
    root_element: 'order',
    draft: 'draft-07',
    attribute_prefix: '$',
    text_property: '#text',
    required_from_occurs: 'true',
    additional_properties: 'true',
    annotations: 'false',
  });
  await page.goto(`/tools/xsd-to-json-schema/?${qs.toString()}`);
  await expect(page.locator('#in-root_element')).toHaveValue('order');
  await expect(page.locator('#in-draft')).toHaveValue('draft-07');
  await expect(page.locator('#in-attribute_prefix')).toHaveValue('$');
  await expect(page.locator('#in-additional_properties')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"$schema": "http://json-schema.org/draft-07/schema#"', { timeout: 15_000 });
  await expect(out).toContainText('"$currency"');
  await expect(out).toContainText('"required"');
  await expect(out).toContainText('"total"');
});
