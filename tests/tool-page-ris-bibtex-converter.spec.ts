import { test, expect } from './fixtures';

const RIS_ARTICLE = [
  'TY  - JOUR',
  'AU  - Shannon, C. E.',
  'TI  - A Mathematical Theory of Communication',
  'JO  - Bell System Technical Journal',
  'PY  - 1948',
  'SP  - 379',
  'EP  - 423',
  'ER  - ',
].join('\n');

const BIB_BOOK = '@book{knuth1984, title = {The {TeX}book}, author = {Knuth, Donald E.}, publisher = {Addison-Wesley}, year = {1984}}';

async function setSource(page: any, value: string) {
  await page.$eval(
    '#in-input',
    (el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value,
  );
}

test('ris-bibtex-converter page converts RIS to BibTeX with generated cite keys', async ({ page }) => {
  await page.goto('/tools/ris-bibtex-converter/');
  await setSource(page, RIS_ARTICLE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('@article{shannon1948mathematical,', { timeout: 15000 });
  await expect(out).toContainText('author = {Shannon, C. E.}');
  await expect(out).toContainText('journal = {Bell System Technical Journal}');
  await expect(out).toContainText('pages = {379--423}');
});

test('ris-bibtex-converter page converts BibTeX to RIS with explicit direction', async ({ page }) => {
  await page.goto('/tools/ris-bibtex-converter/');
  await setSource(page, BIB_BOOK);
  await page.selectOption('#in-direction', 'bibtex-to-ris');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('TY  - BOOK', { timeout: 15000 });
  await expect(out).toContainText('ID  - knuth1984');
  await expect(out).toContainText('AU  - Knuth, Donald E.');
  await expect(out).toContainText('TI  - The TeXbook');
  await expect(out).toContainText('PB  - Addison-Wesley');
});

test('ris-bibtex-converter query params prefill controls and run a compact conversion', async ({ page }) => {
  const input = [
    'TY  - JOUR',
    'AU  - Curie, Marie',
    'TI  - Sur une substance nouvelle radio-active',
    'JO  - Comptes Rendus',
    'PY  - 1898/07//',
    'AB  - A note on a new strongly radio-active substance found in pitchblende.',
    'KW  - radioactivity',
    'ER  - ',
  ].join('\n');

  await page.goto(
    '/tools/ris-bibtex-converter/?input=' +
      encodeURIComponent(input) +
      '&direction=ris-to-bibtex&key_style=numeric&include_abstract=false&include_keywords=false&translate_latex=true&indent=4&sort=source',
  );

  await expect(page.locator('#in-input')).toHaveValue(input, { timeout: 15000 });
  await expect(page.locator('#in-direction')).toHaveValue('ris-to-bibtex');
  await expect(page.locator('#in-key_style')).toHaveValue('numeric');
  await expect(page.locator('#in-include_abstract')).not.toBeChecked();
  await expect(page.locator('#in-include_keywords')).not.toBeChecked();
  await expect(page.locator('#in-indent')).toHaveValue('4');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('@article{ref1,', { timeout: 15000 });
  await expect(out).toContainText('    author = {Curie, Marie}');
  await expect(out).toContainText('    title = {Sur une substance nouvelle radio-active}');
  await expect(out).not.toContainText('abstract');
  await expect(out).not.toContainText('keywords');
});
