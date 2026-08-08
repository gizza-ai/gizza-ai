import { test, expect } from './fixtures';

const output = (page) =>
  page.locator('#tool-output').evaluate((el) => el.textContent?.trim() ?? '');

test('html-to-jsx converts form markup with default React rewrites', async ({ page }) => {
  await page.goto('/tools/html-to-jsx/');
  await page.fill(
    '#in-html',
    '<div class="card"><label for="n">Name</label><input id="n" tabindex="2" readonly></div>'
  );
  await expect(page.locator('#tool-output')).toContainText('className', { timeout: 15000 });
  expect(await output(page)).toBe(
    '<div className="card">\n' +
      '  <label htmlFor="n">Name</label>\n' +
      '  <input id="n" tabIndex={2} readOnly={true} />\n' +
      '</div>'
  );
});

test('html-to-jsx deep-link strips comments and uses boolean shorthand', async ({ page }) => {
  const html = encodeURIComponent('<!-- nav --><button disabled onclick="save()">Go</button>');
  await page.goto(`/tools/html-to-jsx/?html=${html}&comments=strip&boolean_attrs=shorthand&value_attrs=keep`);
  await expect(page.locator('#in-comments')).toHaveValue('strip');
  await expect(page.locator('#in-boolean_attrs')).toHaveValue('shorthand');
  await expect(page.locator('#tool-output')).toContainText('onClick', { timeout: 15000 });
  expect(await output(page)).toBe('<button disabled onClick={() => { save() }}>Go</button>');
});
