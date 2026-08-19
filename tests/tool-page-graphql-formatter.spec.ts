import { test, expect } from './fixtures';

const messyQuery = 'query Hero($episode: Episode = JEDI) { hero(episode: $episode) { name friends { name } } }';

test('graphql-formatter formats a real query', async ({ page }) => {
  await page.goto('/tools/graphql-formatter/');
  await page.fill('#in-input', messyQuery);

  const output = page.locator('#tool-output');
  await expect(output).toContainText('query Hero($episode: Episode = JEDI) {', { timeout: 20000 });
  await expect(output).toContainText('  hero(episode: $episode) {');
  await expect(output).toContainText('    friends {');
});

test('graphql-formatter deep link minifies and sorts fields', async ({ page }) => {
  const qs =
    '?input=' + encodeURIComponent('# note\n{ z y x { c b a } }') +
    '&indent=2' +
    '&mode=minify' +
    '&sort_fields=true' +
    '&remove_comments=false';

  await page.goto('/tools/graphql-formatter/' + qs);
  await expect(page.locator('#in-mode')).toHaveValue('minify', { timeout: 15000 });
  await expect(page.locator('#in-sort_fields')).toBeChecked();

  const output = page.locator('#tool-output');
  await expect(output).toContainText('{x{a b c}y z}', { timeout: 20000 });
  await expect(output).not.toContainText('# note');
});

test('graphql-formatter reports syntax errors', async ({ page }) => {
  await page.goto('/tools/graphql-formatter/');
  await page.fill('#in-input', 'query Q { hero { name }');

  await expect(page.locator('#tool-output')).toHaveClass(/error/, { timeout: 20000 });
  await expect(page.locator('#tool-output')).toContainText('Syntax error at line 1');
});
