import { test, expect } from './fixtures';

const articleHtml = '<article>\n' +
  '  <img src="/hero.jpg" alt="Team on a rooftop" width="1200" height="800" decoding="async" fetchpriority="high">\n' +
  '  <img src="/promo.png" width="600" height="400" loading="lazy">\n' +
  '  <img src="/chart.png" alt="Revenue by quarter" loading="lazy" decoding="async">\n' +
  '</article>';

const pictureHtml = '<picture>\n' +
  '  <source srcset="/hero.avif 1x, /hero@2x.avif 2x" type="image/avif" media="(min-width: 800px)">\n' +
  '  <img src="/hero.jpg" alt="A hero image" width="800" height="600" sizes="100vw" loading="lazy" decoding="async">\n' +
  '</picture>';

test('html-image-inventory page flags missing alt and dimensions', async ({ page }) => {
  await page.goto('/tools/html-image-inventory/');
  await page.fill('#in-html', articleHtml);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('3 images, 1 missing alt, 1 missing dimensions, 2 lazy-loaded', { timeout: 15_000 });
  await expect(out).toContainText('| 1 | img | `/hero.jpg` | Team on a rooftop | 1200×800 | — | async | — |');
  await expect(out).toContainText('| 2 | img | `/promo.png` | — | 600×400 | lazy | — | missing-alt |');
  await expect(out).toContainText('| 3 | img | `/chart.png` | Revenue by quarter | — | lazy | async | missing-width, missing-height |');
});

test('html-image-inventory deep-link renders csv with picture sources and issue filter', async ({ page }) => {
  const qs = new URLSearchParams({
    html: pictureHtml,
    format: 'csv',
    include_sources: 'true',
    only_issues: 'false',
    flag_empty_alt: 'false',
    include_summary: 'true',
  });
  await page.goto(`/tools/html-image-inventory/?${qs.toString()}`);

  await expect(page.locator('#in-html')).toHaveValue(pictureHtml, { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('csv');
  await expect(page.locator('#in-include_sources')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('index,element,picture,src,srcset,sizes,media,type,alt,alt_state,width,height');
  await expect(out).toContainText('1,source,1,,"/hero.avif 1x, /hero@2x.avif 2x",,(min-width: 800px),image/avif,,n/a');
  await expect(out).toContainText('2,img,1,/hero.jpg,,100vw,,,A hero image,present,800,600,lazy,async');

  await page.locator('#in-only_issues').check();
  await expect(out).toContainText('index,element,picture,src,srcset,sizes,media,type,alt,alt_state,width,height', { timeout: 15_000 });
  await expect(out).not.toContainText('/hero.jpg');
});
