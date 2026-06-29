import { test, expect } from './fixtures';

const GOOD = `<html lang="en"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>A Great Page About Widgets and Gadgets</title>
<meta name="description" content="This is a thoroughly written meta description that sits comfortably within the recommended fifty to one hundred sixty character range.">
<link rel="canonical" href="https://example.com/widgets">
<link rel="icon" href="/favicon.ico">
<meta property="og:title" content="Widgets">
<meta property="og:description" content="All about widgets">
<meta property="og:image" content="https://example.com/w.png">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="Widgets">
<meta name="twitter:description" content="All about widgets">
<meta name="twitter:image" content="https://example.com/w.png">
<script type="application/ld+json">{"@context":"https://schema.org","@type":"Article"}</script>
</head><body>
<h1>Widgets</h1><h2>Types</h2><h3>Round</h3>
<img src="a.png" alt="a round widget">
</body></html>`;

const BAD = `<html><body><img src="a.png"><h1>One</h1><h1>Two</h1></body></html>`;

test('seo-audit scores a well-formed page grade A', async ({ page }) => {
  await page.goto('/tools/seo-audit/');
  await page.fill('#in-html', GOOD);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('SEO score:', { timeout: 15000 });
  await expect(out).toContainText('(grade A)');
  await expect(out).toContainText('0 warnings, 0 failed');
  await expect(out).toContainText('✓ Title tag');
  await expect(out).toContainText('✓ Open Graph tags');
});

test('seo-audit flags missing tags on a bare page', async ({ page }) => {
  await page.goto('/tools/seo-audit/');
  await page.fill('#in-html', BAD);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('SEO score:', { timeout: 15000 });
  await expect(out).toContainText('✗ Title tag');
  await expect(out).toContainText('✗ Meta description');
  await expect(out).toContainText('✗ Image alt text');
  await expect(out).toContainText('2 <h1> tags found');
});
