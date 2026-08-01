import { test, expect } from './fixtures';

async function setHtml(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-html').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('html-sanitizer removes scripts, handlers, and unsafe URLs', async ({ page }) => {
  await page.goto('/tools/html-sanitizer/');
  await setHtml(page, '<p onclick="alert(1)">Hello <a href="javascript:alert(1)">world</a></p><script>steal()</script>');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<p>Hello <a>world</a></p>', { timeout: 15_000 });
  expect(await out.textContent()).toBe('<p>Hello <a>world</a></p>');
});

test('html-sanitizer supports plain-text mode', async ({ page }) => {
  await page.goto('/tools/html-sanitizer/');
  await setHtml(page, '<h1>Title</h1><p>Hello <strong>world</strong>.</p><style>p{color:red}</style>');
  await page.selectOption('#in-mode', 'plain-text');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Title', { timeout: 15_000 });
  const text = await out.textContent();
  expect(text).toContain('Hello world.');
  expect(text).not.toContain('<');
  expect(text).not.toContain('color:red');
});

test('html-sanitizer deep-link applies checkbox controls', async ({ page }) => {
  const qs = new URLSearchParams({
    html: '<p class="MsoNormal" id="x"><img src="https://example.test/a.png" alt="a">Body</p>',
    mode: 'safe-html',
    allow_images: 'false',
    keep_classes: 'false',
  });
  await page.goto(`/tools/html-sanitizer/?${qs.toString()}`);

  await expect(page.locator('#in-html')).toHaveValue('<p class="MsoNormal" id="x"><img src="https://example.test/a.png" alt="a">Body</p>');
  await expect(page.locator('#in-allow_images')).not.toBeChecked();
  await expect(page.locator('#in-keep_classes')).not.toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<p>Body</p>', { timeout: 15_000 });
  expect(await out.textContent()).toBe('<p>Body</p>');
});

test('html-sanitizer can keep safe inline styles when enabled', async ({ page }) => {
  await page.goto('/tools/html-sanitizer/');
  await setHtml(page, '<p style="color:red">ok</p><p style="background:url(javascript:alert(1))">bad</p>');
  await page.locator('#in-allow_styles').check();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<p style="color:red">ok</p><p>bad</p>', { timeout: 15_000 });
  expect(await out.textContent()).toBe('<p style="color:red">ok</p><p>bad</p>');
});
