import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function outputText(page: import('@playwright/test').Page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const CURIE = `@article{curie1898,
  title   = {Sur une substance nouvelle radio-active},
  author  = {Curie, Marie and Curie, Pierre},
  journal = {Comptes Rendus},
  year    = {1898},
  volume  = {127},
  pages   = {175--178}
}`;

const STANDARD = `type,key,title,author,year,journal,booktitle,volume,number,pages,publisher,doi,isbn,issn,url
article,curie1898,Sur une substance nouvelle radio-active,"Curie, Marie and Curie, Pierre",1898,Comptes Rendus,,127,,175–178,,,,,`;

test('bibtex-to-csv page emits exact standard CSV', async ({ page }) => {
  await page.goto('/tools/bibtex-to-csv/');
  await setField(page, '#in-bibtex', CURIE);

  await expect(page.locator('#tool-output')).toContainText('curie1898', { timeout: 15_000 });
  expect(await outputText(page)).toBe(STANDARD);
});

test('bibtex-to-csv deep-link fills params and renders custom author CSV', async ({ page }) => {
  const params = new URLSearchParams({
    bibtex: '@article{smith2020, title={A study}, author={John Smith and Ludwig van Beethoven}, year={2020}}',
    columns: 'custom',
    custom_columns: 'key,author,year,title',
    delimiter: 'semicolon',
    header: 'false',
    decode_latex: 'true',
    author_format: 'last-first',
    author_separator: 'pipe',
    expand_strings: 'true',
    sort: 'source',
    bom: 'false',
  });
  await page.goto(`/tools/bibtex-to-csv/?${params.toString()}`);

  await expect(page.locator('#in-columns')).toHaveValue('custom', { timeout: 15_000 });
  await expect(page.locator('#in-delimiter')).toHaveValue('semicolon');
  await expect(page.locator('#in-header')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('smith2020', { timeout: 15_000 });
  expect(await outputText(page)).toBe('smith2020;Smith, John | van Beethoven, Ludwig;2020;A study');
});

test('bibtex-to-csv supports all column, tab delimiter and string expansion choices', async ({ page }) => {
  await page.goto('/tools/bibtex-to-csv/');
  await setField(
    page,
    '#in-bibtex',
    '@string{jcp = "J. Chem. Phys."}\n@book{b, title={Book}, isbn={1}}\n@article{a, journal=jcp # " (Letters)", note={N}}',
  );
  await page.selectOption('#in-columns', 'all');
  await page.selectOption('#in-delimiter', 'tab');
  await page.selectOption('#in-sort', 'key');

  await expect(page.locator('#tool-output')).toContainText('J. Chem. Phys. (Letters)', {
    timeout: 15_000,
  });
  expect(await outputText(page)).toBe(
    'type\tkey\tisbn\tjournal\tnote\ttitle\narticle\ta\t\tJ. Chem. Phys. (Letters)\tN\t\nbook\tb\t1\t\t\tBook',
  );

  await page.uncheck('#in-expand_strings');
  await expect(page.locator('#tool-output')).toContainText('jcp (Letters)', { timeout: 15_000 });
});

test('bibtex-to-csv handles Excel BOM and shows a runnable CLI example', async ({ page }) => {
  await page.goto('/tools/bibtex-to-csv/');
  await setField(page, '#in-bibtex', '@article{munoz2021, title={Mesures}, author={Mu\\~noz, Jos\\\'e}, year={2021}}');
  await page.selectOption('#in-delimiter', 'pipe');
  await page.check('#in-bom');

  await expect(page.locator('#tool-output')).toContainText('mu', { timeout: 15_000, ignoreCase: true });
  const raw = (await page.locator('#tool-output').textContent()) ?? '';
  expect(raw.charCodeAt(0)).toBe(0xfeff);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool bibtex-to-csv');
  expect(cli).toContain('curie1898');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
