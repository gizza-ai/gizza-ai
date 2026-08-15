import { test, expect } from './fixtures';

const PHISH = `From: "PayPal Security" <alerts@paypa1-secure.com>
Subject: Urgent

<p><a href="http://192.0.2.9/login">https://www.paypal.com/signin</a></p>
<p><a href="https://www.paypal.com/help">Help centre</a></p>`;

const SUMMARY = `Phishing link scan: CRITICAL (score 78/100)
Links: 2 scanned, 1 flagged

CRITICAL (78/100) http://192.0.2.9/login — Link text shows 'paypal.com' but the link actually goes to '2.9'.
`;

async function runWasm(
  page: import('@playwright/test').Page,
  overrides: Partial<Record<string, string>> = {},
) {
  const args = {
    email: PHISH,
    brands: '',
    format: 'auto',
    report: 'summary',
    onlyFlagged: 'false',
    maxLinks: '10',
    ...overrides,
  };
  return page.evaluate(async (args) => {
    const mod = await import('/tools/email-phishing-link-scanner/gizza_ai_email_phishing_link_scanner_web.js');
    await mod.default('/tools/email-phishing-link-scanner/gizza_ai_email_phishing_link_scanner_web_bg.wasm');
    return mod.run(args.email, args.brands, args.format, args.report, args.onlyFlagged, args.maxLinks);
  }, args);
}

test('email-phishing-link-scanner page renders exact summary from form values', async ({ page }) => {
  await page.goto('/tools/email-phishing-link-scanner/');
  await page.fill('#in-email', PHISH);
  await page.selectOption('#in-report', 'summary');
  await page.fill('#in-max_links', '10');

  await expect(page.locator('#tool-output')).toContainText('Phishing link scan: CRITICAL', { timeout: 15_000 });
  expect((await page.locator('#tool-output').textContent())!.trim()).toBe(SUMMARY.trim());
});

test('email-phishing-link-scanner deep link drives json output and non-default checkbox', async ({ page }) => {
  const params = new URLSearchParams({
    email: PHISH,
    report: 'json',
    only_flagged: 'true',
    max_links: '10',
  });
  await page.goto(`/tools/email-phishing-link-scanner/?${params.toString()}`);

  await expect(page.locator('#in-report')).toHaveValue('json');
  await expect(page.locator('#in-only_flagged')).toBeChecked();
  await expect(page.locator('#in-max_links')).toHaveValue('10');
  await expect(page.locator('#tool-output')).toContainText('"rating": "CRITICAL"', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"links_flagged": 1');
});

test('email-phishing-link-scanner wasm covers advertised options and boundaries', async ({ page }) => {
  await page.goto('/tools/email-phishing-link-scanner/');
  await page.waitForSelector('#in-email');

  expect(await runWasm(page)).toBe(SUMMARY);

  const json = JSON.parse(await runWasm(page, { report: 'json', onlyFlagged: 'true', brands: 'paypal.com' }));
  expect(json.rating).toBe('CRITICAL');
  expect(json.links_scanned).toBe(2);
  expect(json.links_flagged).toBe(1);
  expect(json.links).toHaveLength(1);

  const detailed = await runWasm(page, { report: 'detailed', format: 'html', maxLinks: '1' });
  expect(detailed).toContain('2 links found, only the first 1 were scanned');

  await expect(runWasm(page, { report: 'nope' })).rejects.toThrow(/unknown report/);
  await expect(runWasm(page, { maxLinks: 'not-a-number' })).rejects.toThrow(/max_links must be a whole number/);
});

test('email-phishing-link-scanner generated CLI example is generic and brand-free', async ({ page }) => {
  await page.goto('/tools/email-phishing-link-scanner/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool email-phishing-link-scanner');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
