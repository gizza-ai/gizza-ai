import { test, expect } from './fixtures';

async function svgFromOutput(page): Promise<string> {
  const img = page.locator('#tool-output-media');
  await expect(img).toBeVisible({ timeout: 15000 });
  const src = await img.getAttribute('src');
  expect(src?.startsWith('data:image/svg+xml;base64,')).toBe(true);
  return Buffer.from(src!.slice('data:image/svg+xml;base64,'.length), 'base64').toString('utf8');
}

test('vcard-qr renders a real contact-card QR SVG', async ({ page }) => {
  await page.goto('/tools/vcard-qr/');
  await page.fill('#in-first_name', 'Ada');
  await page.fill('#in-last_name', 'Lovelace');
  await page.fill('#in-organization', 'Analytical Engines');
  await page.fill('#in-job_title', 'Chief Analyst');
  await page.fill('#in-mobile', '+44 7700 900123');
  await page.fill('#in-email', 'ada@example.com');
  await page.fill('#in-website', 'example.com/ada');
  await page.selectOption('#in-version', '3.0');
  await page.selectOption('#in-error_correction', 'M');
  await page.fill('#in-size', '256');
  await page.fill('#in-foreground', '#000000');
  await page.fill('#in-background', '#ffffff');

  const svg = await svgFromOutput(page);
  expect(svg).toContain('<svg xmlns="http://www.w3.org/2000/svg"');
  expect(svg).toContain('width="256"');
  expect(svg).toContain('<title>Contact QR code for Ada Lovelace</title>');
  expect(svg).toContain('BEGIN:VCARD');
  expect(svg).toContain('VERSION:3.0');
  expect(svg).toContain('FN:Ada Lovelace');
  expect(svg).toContain('ORG:Analytical Engines');
  expect(svg).toContain('TITLE:Chief Analyst');
  expect(svg).toContain('TEL;TYPE=CELL:+44 7700 900123');
  expect(svg).toContain('EMAIL;TYPE=INTERNET:ada@example.com');
  expect(svg).toContain('URL:https://example.com/ada');
  expect(svg).toContain('data-role="contact-caption"');
  await expect(page.locator('#tool-output-download')).toBeVisible();
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'vcard-qr.svg');
});

test('vcard-qr supports deep links, vCard 4.0, colours and no-caption mode', async ({ page }) => {
  await page.goto(
    '/tools/vcard-qr/?first_name=Grace&last_name=Hopper&organization=Navy&mobile=%2B1%20202%20555%200142&email=grace%40example.com&birthday=1906-12-09&version=4.0&error_correction=H&size=384&foreground=%23111827&background=%23fff&show_details=false',
  );

  await expect(page.locator('#in-first_name')).toHaveValue('Grace', { timeout: 15000 });
  await expect(page.locator('#in-show_details')).not.toBeChecked();
  const svg = await svgFromOutput(page);
  expect(svg).toContain('width="384"');
  expect(svg).toContain('VERSION:4.0');
  expect(svg).toContain('FN:Grace Hopper');
  expect(svg).toContain('TEL;TYPE=cell:+1 202 555 0142');
  expect(svg).toContain('BDAY:1906-12-09');
  expect(svg).toContain('fill="#111827"');
  expect(svg).toContain('fill="#fff"');
  expect(svg).not.toContain('data-role="contact-caption"');
});

test('vcard-qr clears stale SVG and download link on validation error', async ({ page }) => {
  await page.goto('/tools/vcard-qr/');
  await page.fill('#in-first_name', 'Ada');
  await page.fill('#in-email', 'ada@example.com');
  await expect(page.locator('#tool-output-media')).toBeVisible({ timeout: 15000 });
  await expect(page.locator('#tool-output-download')).toBeVisible();

  await page.fill('#in-email', 'not-an-email');
  await expect(page.locator('#tool-output')).toContainText('not a valid address', { timeout: 15000 });
  await expect(page.locator('#tool-output-media')).toBeHidden();
  await expect(page.locator('#tool-output-download')).toBeHidden();
});
