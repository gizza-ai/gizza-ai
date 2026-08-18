import { test, expect } from './fixtures';

async function setValue(page: import('@playwright/test').Page, selector: string, value: string) {
  await page.locator(selector).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

const basicSections = 'heading | What shipped in March\ntext | Hi {{first_name}}, three things went live.\nbutton | Read more | https://example.com/changelog\nfooter | [Unsubscribe]({{unsubscribe_url}})';

test('newsletter-html-builder renders table-safe newsletter html', async ({ page }) => {
  await page.goto('/tools/newsletter-html-builder/');
  await setValue(page, '#in-sections', basicSections);
  await setValue(page, '#in-subject', 'March newsletter');
  await setValue(page, '#in-preheader', 'Three product updates inside');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<title>March newsletter</title>', { timeout: 15_000 });
  await expect(out).toContainText('role="presentation"');
  await expect(out).toContainText('mso-hide:all');
  await expect(out).toContainText('bgcolor="#2563eb"');
  await expect(out).toContainText('href="https://example.com/changelog"');
  await expect(out).toContainText('href="{{unsubscribe_url}}"');
});

test('newsletter-html-builder deep-links themed output and disabled dark mode', async ({ page }) => {
  const qs = new URLSearchParams({
    sections: 'heading | Sale starts now\nbutton | Shop now | https://example.com/shop',
    subject: 'Sale announcement',
    preheader: 'Save today',
    width: '480',
    background: '#fff7ed',
    content_background: '#ffffff',
    text_color: '#111827',
    accent: '#ea580c',
    font: 'georgia',
    dark_mode: 'false',
  });
  await page.goto(`/tools/newsletter-html-builder/?${qs.toString()}`);

  await expect(page.locator('#in-sections')).toHaveValue('heading | Sale starts now\nbutton | Shop now | https://example.com/shop', { timeout: 15_000 });
  await expect(page.locator('#in-width')).toHaveValue('480');
  await expect(page.locator('#in-background')).toHaveValue('#fff7ed');
  await expect(page.locator('#in-accent')).toHaveValue('#ea580c');
  await expect(page.locator('#in-font')).toHaveValue('georgia');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('width:480px;max-width:480px', { timeout: 15_000 });
  await expect(out).toContainText('font-family:Georgia');
  await expect(out).toContainText('bgcolor="#ea580c"');
  await expect(out).not.toContainText('prefers-color-scheme');
});

test('newsletter-html-builder covers color forms and font enum choices', async ({ page }) => {
  await page.goto('/tools/newsletter-html-builder/');
  await setValue(page, '#in-sections', 'text | Hello');
  await setValue(page, '#in-width', '320');
  await setValue(page, '#in-background', 'white');
  await setValue(page, '#in-accent', '#f00');
  await page.selectOption('#in-font', 'courier');
  await page.locator('#in-dark_mode').uncheck();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('width:320px;max-width:320px', { timeout: 15_000 });
  await expect(out).toContainText('background-color:white');
  await expect(out).toContainText("font-family:'Courier New', Courier, monospace");
});
