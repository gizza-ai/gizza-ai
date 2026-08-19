import { test, expect } from './fixtures';

async function svgFromOutput(page): Promise<string> {
  const img = page.locator('#tool-output-media');
  await expect(img).toBeVisible({ timeout: 15000 });
  const src = await img.getAttribute('src');
  expect(src?.startsWith('data:image/svg+xml;base64,')).toBe(true);
  return Buffer.from(src!.slice('data:image/svg+xml;base64,'.length), 'base64').toString('utf8');
}

test('renders a Bitcoin BIP21 payment QR SVG with exact URI text', async ({ page }) => {
  await page.goto('/tools/payment-qr/');
  await page.fill('#in-address', 'bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4');
  await page.selectOption('#in-scheme', 'bitcoin');
  await page.fill('#in-amount', '0.025');
  await page.fill('#in-label', 'Coffee Bar');
  await page.fill('#in-message', 'Invoice 2026-114');
  await page.selectOption('#in-error_correction', 'M');
  await page.fill('#in-size', '256');
  await page.fill('#in-foreground', '#000000');
  await page.fill('#in-background', '#ffffff');
  await expect(page.locator('#tool-output-download')).toBeVisible({ timeout: 15000 });
  const svg = await svgFromOutput(page);
  const uri = 'bitcoin:bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4?amount=0.025&amp;label=Coffee%20Bar&amp;message=Invoice%202026-114';
  expect(svg).toContain('<svg xmlns="http://www.w3.org/2000/svg"');
  expect(svg).toContain(uri);
  expect(svg).toContain('width="256"');
  expect(svg).toContain('#000000');
});

test('supports text deep links and non-default checkbox state', async ({ page }) => {
  await page.goto(
    '/tools/payment-qr/?address=' +
      encodeURIComponent('pay me at the front desk') +
      '&scheme=text&error_correction=H&size=384&foreground=%23111111&background=%23ffffff&show_uri=false',
  );
  const svg = await svgFromOutput(page);
  expect(svg).toContain('<title>pay me at the front desk</title>');
  expect(svg).toContain('width="384"');
  expect(svg).not.toContain('Payment URI');
});
