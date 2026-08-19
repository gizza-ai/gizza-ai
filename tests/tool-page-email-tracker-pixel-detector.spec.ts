import { test, expect } from './fixtures';

const tool = '/tools/email-tracker-pixel-detector/';
const pixelHtml = '<img src="https://track.hubspot.com/open.gif?email=a@example.com&id=abc123456789abcdef" width="1" height="1" style="display:none">';
const customPixel = '<img src="https://img.newsletter.example/pixel.gif?uid=abc123456789abcdef" width="1" height="1">';
const cleanEmbedded = '<img src="cid:logo@example" width="600" height="80" alt="Logo">';
const clickTracked = '<a href="https://mailchimp.com/track/click?u=abcdef0123456789abcdef">Read</a>';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  text: string,
  format = 'html',
  report = 'summary',
  includeLinks = 'false',
  vendors = '',
): Promise<string> {
  return await page.evaluate(
    async ({ text, format, report, includeLinks, vendors }) => {
      const mod = await import('/tools/email-tracker-pixel-detector/gizza_ai_email_tracker_pixel_detector_web.js');
      await mod.default('/tools/email-tracker-pixel-detector/gizza_ai_email_tracker_pixel_detector_web_bg.wasm');
      return mod.run(text, format, report, includeLinks, vendors);
    },
    { text, format, report, includeLinks, vendors },
  );
}

test('email-tracker-pixel-detector page reports a hidden vendor pixel', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-text'), pixelHtml);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Verdict: TRACKED', { timeout: 15_000 });
  await expect(out).toContainText('track.hubspot.com');
  await expect(out).toContainText('tiny-pixel');
  await expect(out).toContainText('vendor=HubSpot');
});

test('email-tracker-pixel-detector deep link can render hosts output with custom vendor', async ({ page }) => {
  const qs = new URLSearchParams({
    text: customPixel,
    format: 'html',
    report: 'hosts',
    vendors: 'newsletter.example',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-text')).toHaveValue(customPixel, { timeout: 15_000 });
  await expect(page.locator('#in-report')).toHaveValue('hosts');
  await expect(page.locator('#in-vendors')).toHaveValue('newsletter.example');
  await expect(page.locator('#tool-output')).toHaveText(/img\.newsletter\.example/);
});

test('email-tracker-pixel-detector wasm covers report enum, checkbox state, custom vendors, and validation', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-text');

  const clean = await runWasm(page, cleanEmbedded, 'html', 'summary', 'false', '');
  expect(clean).toContain('Verdict: CLEAN');
  expect(clean).toContain('embedded: 1');

  const json = JSON.parse(await runWasm(page, customPixel, 'html', 'json', 'false', 'newsletter.example'));
  expect(json.verdict).toBe('TRACKED');
  expect(json.assets[0].vendor).toBe('custom:newsletter.example');

  const hosts = await runWasm(page, pixelHtml, 'html', 'hosts', 'false', '');
  expect(hosts.trim()).toBe('track.hubspot.com');

  const linksOff = await runWasm(page, clickTracked, 'html', 'summary', 'false', '');
  expect(linksOff).toContain('Verdict: CLEAN');
  const linksOn = await runWasm(page, clickTracked, 'html', 'summary', 'true', '');
  expect(linksOn).toContain('Verdict: TRACKED');
  expect(linksOn).toContain('click-tracker');

  await expect(runWasm(page, pixelHtml, 'xml', 'summary', 'false', '')).rejects.toThrow(/unknown format/);
  await expect(runWasm(page, pixelHtml, 'html', 'table', 'false', '')).rejects.toThrow(/unknown report/);
});

test('email-tracker-pixel-detector ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(3);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Hidden vendor pixel',
    'Hosts for a blocklist',
    'Clean embedded logo',
  ]);
});
