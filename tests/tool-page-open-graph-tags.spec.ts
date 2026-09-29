import { test, expect } from './fixtures';

const title = 'How to bake sourdough';
const description = 'A step-by-step sourdough guide covering starter, autolyse, bulk ferment and bake.';
const url = 'https://example.com/sourdough';
const image = 'https://example.com/og/sourdough.png';

test('open-graph-tags generates full rich-preview markup', async ({ page }) => {
  await page.goto('/tools/open-graph-tags/');
  await page.fill('#in-title', title);
  await page.fill('#in-description', description);
  await page.fill('#in-url', url);
  await page.fill('#in-image', image);
  await page.fill('#in-image_alt', 'A sliced sourdough loaf');
  await page.fill('#in-image_width', '1200');
  await page.fill('#in-image_height', '630');
  await page.fill('#in-site_name', 'Example Bakery');
  await page.selectOption('#in-og_type', 'article');
  await page.fill('#in-author', 'Dana Ruiz');
  await page.fill('#in-twitter_site', 'examplebakery');
  await page.fill('#in-twitter_creator', 'https://x.com/some_baker/');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<title>How to bake sourdough</title>', { timeout: 15000 });
  await expect(out).toContainText('<meta property="og:type" content="article">');
  await expect(out).toContainText('<meta property="article:author" content="Dana Ruiz">');
  await expect(out).toContainText('<meta name="twitter:site" content="@examplebakery">');
  await expect(out).toContainText('<meta name="twitter:creator" content="@some_baker">');
  await expect(out).toContainText('* No issues found.');
});

test('open-graph-tags query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/open-graph-tags/?title=' +
      encodeURIComponent('Cast iron dutch oven, 5.5 qt') +
      '&description=' +
      encodeURIComponent('An enamelled cast iron dutch oven for bread, braises and stews, with a 5.5 quart capacity.') +
      '&url=' +
      encodeURIComponent('https://example.com/shop/dutch-oven') +
      '&image=' +
      encodeURIComponent('https://example.com/og/dutch-oven.jpg') +
      '&image_width=1200&image_height=630&og_type=product&twitter_card=summary_large_image&include_schema=true',
  );
  await expect(page.locator('#in-og_type')).toHaveValue('product', { timeout: 15000 });
  await expect(page.locator('#in-include_schema')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('<meta property="og:type" content="product">', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('<meta itemprop="name" content="Cast iron dutch oven, 5.5 qt">');
});

test('open-graph-tags checkbox options can emit Open Graph only without comments', async ({ page }) => {
  await page.goto('/tools/open-graph-tags/');
  await page.fill('#in-title', title);
  await page.fill('#in-description', description);
  await page.fill('#in-url', url);
  await page.uncheck('#in-include_basic');
  await page.uncheck('#in-include_twitter');
  await page.uncheck('#in-group_comments');
  await page.uncheck('#in-warnings');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<meta property="og:title" content="How to bake sourdough">', {
    timeout: 15000,
  });
  await expect(out).not.toContainText('<title>');
  await expect(out).not.toContainText('twitter:');
  await expect(out).not.toContainText('<!--');
});

test('open-graph-tags escapes attribute values and validates the dimension cap', async ({ page }) => {
  await page.goto('/tools/open-graph-tags/');
  await page.fill('#in-title', 'Tom & Jerry\'s "best" <hits>');
  await page.fill('#in-description', description);
  await expect(page.locator('#tool-output')).toContainText(
    '<meta property="og:title" content="Tom &amp; Jerry&#39;s &quot;best&quot; &lt;hits&gt;">',
    { timeout: 15000 },
  );

  await page.fill('#in-image_width', '10001');
  await expect(page.getByRole('status')).toContainText('image_width must be between 0 and 10000 pixels', {
    timeout: 15000,
  });
});
