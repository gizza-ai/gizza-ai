import { test, expect } from './fixtures';

const PAGE = '<html><body><nav>Home About</nav><div class=ad>BUY NOW</div><article><h1>Real Headline</h1><p>This is the substantial article paragraph with enough real prose that the readability algorithm treats it as the main content of the page.</p></article><footer>copyright</footer></body></html>';

test('readability-extractor page pulls the article, drops chrome', async ({ page }) => {
  await page.goto('/tools/readability-extractor/');
  await page.fill('#in-html', PAGE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('substantial article paragraph', { timeout: 15000 });
  await expect(out).not.toContainText('BUY NOW');
});

test('readability-extractor query-param deep-link', async ({ page }) => {
  await page.goto('/tools/readability-extractor/?html=' + encodeURIComponent(PAGE));
  await expect(page.locator('#in-html')).toHaveValue(PAGE, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Real Headline', { timeout: 15000 });
});
