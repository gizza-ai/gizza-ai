import { test, expect } from './fixtures';

const conflict = `const port = 1;
<<<<<<< HEAD
const host = "0.0.0.0";
=======
const host = "127.0.0.1";
>>>>>>> feature/local
start();`;

const twoConflicts = `intro
<<<<<<< HEAD
alpha-ours
=======
alpha-theirs
>>>>>>> topic
middle
<<<<<<< HEAD
beta-ours
=======
beta-theirs
>>>>>>> topic
end`;

const output = (page) =>
  page.locator('#tool-output').evaluate((el) => el.textContent?.trim() ?? '');

test('merge-conflict-resolver page keeps ours by default', async ({ page }) => {
  await page.goto('/tools/merge-conflict-resolver/');
  await page.fill('#in-text', conflict);

  await expect(page.locator('#tool-output')).toContainText('0.0.0.0', { timeout: 15000 });
  expect(await output(page)).toBe('const port = 1;\nconst host = "0.0.0.0";\nstart();');
});

test('merge-conflict-resolver deep-link applies per-conflict overrides', async ({ page }) => {
  const qs = new URLSearchParams({
    text: twoConflicts,
    strategy: 'ours',
    choices: '2=theirs',
    output: 'resolved',
    strict: 'false',
  });
  await page.goto(`/tools/merge-conflict-resolver/?${qs.toString()}`);

  await expect(page.locator('#in-strategy')).toHaveValue('ours');
  await expect(page.locator('#in-choices')).toHaveValue('2=theirs');
  await expect(page.locator('#tool-output')).toContainText('beta-theirs', { timeout: 15000 });
  expect(await output(page)).toBe('intro\nalpha-ours\nmiddle\nbeta-theirs\nend');
});

test('merge-conflict-resolver page lists conflicts and supports non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/merge-conflict-resolver/');
  await page.fill('#in-text', twoConflicts);
  await page.selectOption('#in-output', 'list');
  await page.check('#in-strict');

  await expect(page.locator('#in-strict')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('[2] lines 8-12', { timeout: 15000 });
  expect(await output(page)).toBe(
    '2 conflicts · 2 resolved\n\n' +
      '[1] lines 2-6 · ours "HEAD" 1 line · theirs "topic" 1 line → ours\n' +
      '[2] lines 8-12 · ours "HEAD" 1 line · theirs "topic" 1 line → ours',
  );
});

test('merge-conflict-resolver page reports base errors for ordinary conflicts', async ({ page }) => {
  await page.goto('/tools/merge-conflict-resolver/');
  await page.fill('#in-text', conflict);
  await page.selectOption('#in-strategy', 'base');

  await expect(page.locator('#tool-output')).toContainText("has no '|||||||' common-ancestor section", {
    timeout: 15000,
  });
});
