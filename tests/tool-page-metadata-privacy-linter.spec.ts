import { test, expect } from './fixtures';

const XMP_IMAGE_BASE64 =
  'iVBORw0KGgpwcmVmaXg8eDp4bXBtZXRhIHhtbG5zOng9J2Fkb2JlOm5zOm1ldGEvJz48cmRmOlJERj48cmRmOkRlc2NyaXB0aW9uIGF1eDpTZXJpYWxOdW1iZXI9IlNOMTIzNDUiIHhtcDpDcmVhdG9yVG9vbD0iR0lNUCAyLjEwIj48ZGM6Y3JlYXRvcj48cmRmOlNlcT48cmRmOmxpPkFkYSBMb3ZlbGFjZTwvcmRmOmxpPjwvcmRmOlNlcT48L2RjOmNyZWF0b3I+PC9yZGY6RGVzY3JpcHRpb24+PC9yZGY6UkRGPjwveDp4bXBtZXRhPnN1ZmZpeA==';

test('metadata-privacy-linter page reports redacted XMP privacy leaks by default', async ({ page }) => {
  await page.goto('/tools/metadata-privacy-linter/');
  await page.fill('#in-image_base64', XMP_IMAGE_BASE64);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('metadata privacy report', { timeout: 15000 });
  await expect(output).toContainText('Findings: 3');
  await expect(output).toContainText('Values hidden: true');
  await expect(output).toContainText('[high] xmp Device serial number (device)');
  await expect(output).toContainText('[high] xmp Creator (personal)');
  await expect(output).not.toContainText('SN12345');
  await expect(output).not.toContainText('Ada Lovelace');
});

test('metadata-privacy-linter query-param deep-link prefills and reveals JSON values', async ({ page }) => {
  const qs = new URLSearchParams({
    image_base64: XMP_IMAGE_BASE64,
    min_risk: 'high',
    reveal_values: 'true',
    output: 'json',
  });
  await page.goto('/tools/metadata-privacy-linter/?' + qs.toString());

  await expect(page.locator('#in-image_base64')).toHaveValue(XMP_IMAGE_BASE64, { timeout: 15000 });
  await expect(page.locator('#in-min_risk')).toHaveValue('high');
  await expect(page.locator('#in-reveal_values')).toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('json');

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"findings_count": 2', { timeout: 15000 });
  await expect(output).toContainText('"value": "SN12345"');
  await expect(output).toContainText('"value": "Ada Lovelace"');
  await expect(output).not.toContainText('Creator tool / software');
});
