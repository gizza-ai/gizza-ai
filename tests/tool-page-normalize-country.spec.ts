import { test, expect } from './fixtures';

const MESSY = ['usa', 'Deutschland', 'Korea, Republic of', '826', 'Swizerland', 'Atlantis'].join('\n');

async function setCountries(page: any, value: string) {
  await page.$eval(
    '#in-input',
    (el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value,
  );
}

test('normalize-country page resolves mixed country names and codes to the default table', async ({ page }) => {
  await page.goto('/tools/normalize-country/');
  await setCountries(page, MESSY);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('usa', { timeout: 15000 });
  await expect(out).toContainText('United States of America');
  await expect(out).toContainText('DE       DEU      276      alias');
  await expect(out).toContainText('Korea (Republic of)');
  await expect(out).toContainText('GB       GBR      826      exact');
  await expect(out).toContainText('Switzerland');
  await expect(out).toContainText('fuzzy');
  await expect(out).toContainText('Atlantis');
  await expect(out).toContainText('unmatched');
});

test('normalize-country page emits alpha-2 codes and can disable fuzzy matching', async ({ page }) => {
  await page.goto('/tools/normalize-country/');
  await setCountries(page, MESSY);
  await page.selectOption('#in-output', 'alpha2');
  await page.uncheck('#in-fuzzy');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('US\nDE\nKR\nGB\nSwizerland\nAtlantis', { timeout: 15000 });
});

test('normalize-country query params prefill controls and audit unresolved rows', async ({ page }) => {
  await page.goto(
    '/tools/normalize-country/?input=' +
      encodeURIComponent('Kosovo\nUSA\nSoviet Union\nJapan\nAtlantis') +
      '&output=alpha3&name_style=common&delimiter=newline&on_unmatched=only&dedupe=true&sort=asc&fuzzy=false',
  );

  await expect(page.locator('#in-input')).toHaveValue('Kosovo\nUSA\nSoviet Union\nJapan\nAtlantis', { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('alpha3');
  await expect(page.locator('#in-name_style')).toHaveValue('common');
  await expect(page.locator('#in-delimiter')).toHaveValue('newline');
  await expect(page.locator('#in-on_unmatched')).toHaveValue('only');
  await expect(page.locator('#in-dedupe')).toBeChecked();
  await expect(page.locator('#in-sort')).toHaveValue('asc');
  await expect(page.locator('#in-fuzzy')).not.toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Atlantis\nKosovo\nSoviet Union', { timeout: 15000 });
  await expect(out).not.toContainText('USA');
  await expect(out).not.toContainText('JPN');
});
