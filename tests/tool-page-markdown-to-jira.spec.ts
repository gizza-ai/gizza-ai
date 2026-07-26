import { test, expect } from './fixtures';

const md = '# Title\n\n**Bold** and `code`\n\n- one\n- two';

test('markdown-to-jira converts Markdown to exact Jira wiki markup', async ({ page }) => {
  await page.goto('/tools/markdown-to-jira/');
  await page.fill('#in-input', md);
  await page.selectOption('#in-direction', 'md-to-jira');
  await page.fill('#in-heading_offset', '0');
  await page.check('#in-panel_blockquotes');

  await expect(page.locator('#tool-output')).toHaveText(
    'h1. Title\n\n*Bold* and {{code}}\n\n* one\n* two',
    { timeout: 15000 },
  );
});

test('markdown-to-jira supports deep-link params, heading offset, and panels', async ({ page }) => {
  const input = '# API changes\n\n> Warning: Breaking field rename';
  await page.goto(
    '/tools/markdown-to-jira/?input=' +
      encodeURIComponent(input) +
      '&direction=md-to-jira&heading_offset=1&panel_blockquotes=true',
  );

  await expect(page.locator('#in-input')).toHaveValue(input, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText(
    'h2. API changes\n\n{warning}\nBreaking field rename\n{warning}',
    { timeout: 15000 },
  );
});

test('markdown-to-jira converts Jira wiki markup back to Markdown with panel conversion off', async ({ page }) => {
  const jira = 'h2. Release notes\n\n*Bold* and {{code}}\n\n# one\n# two';
  await page.goto('/tools/markdown-to-jira/');
  await page.fill('#in-input', jira);
  await page.selectOption('#in-direction', 'jira-to-md');
  await page.fill('#in-heading_offset', '0');
  await page.uncheck('#in-panel_blockquotes');

  await expect(page.locator('#tool-output')).toHaveText(
    '## Release notes\n\n**Bold** and `code`\n\n1. one\n1. two',
    { timeout: 15000 },
  );
});
