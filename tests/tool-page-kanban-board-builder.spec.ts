import { test, expect } from './fixtures';

const STANDUP =
  'Draft the launch email @alice due:2026-09-01\n' +
  'Fix the login redirect — blocked\n' +
  'Rewrite the parser (wip) !high\n' +
  '- [x] Ship the v1 changelog\n' +
  'Migrate the staging database #infra';

const EXPECTED_MARKDOWN =
  '# Kanban Board\n\n' +
  '## To Do (2)\n\n' +
  '- [ ] Draft the launch email @alice due:2026-09-01\n' +
  '- [ ] Migrate the staging database #infra\n\n' +
  '## In Progress (2)\n\n' +
  '- [ ] Fix the login redirect\n' +
  '- [ ] Rewrite the parser !high\n\n' +
  '## Done (1)\n\n' +
  '- [x] Ship the v1 changelog';

async function fillStandup(page: import('@playwright/test').Page) {
  await page.goto('/tools/kanban-board-builder/');
  await page.fill('#in-tasks', STANDUP);
}

test('kanban-board-builder renders exact Markdown sections from standup notes', async ({ page }) => {
  await fillStandup(page);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('## In Progress (2)', { timeout: 15_000 });
  expect(await out.textContent()).toBe(EXPECTED_MARKDOWN);
});

test('kanban-board-builder covers table/json formats and non-default checkbox state', async ({ page }) => {
  await fillStandup(page);
  await page.selectOption('#in-format', 'table');
  await page.uncheck('#in-show_counts');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('| To Do | In Progress | Done |', { timeout: 15_000 });
  await expect(out).toContainText('| Draft the launch email @alice due:2026-09-01 | Fix the login redirect | ~~Ship the v1 changelog~~ |');
  await expect(out).not.toContainText('(2)');

  await page.selectOption('#in-format', 'json');
  await expect(out).toContainText('"total_cards": 5', { timeout: 15_000 });
  await expect(out).toContainText('"name": "In Progress"');
  await expect(out).toContainText('"priority": "high"');
  await expect(out).toContainText('"tags": [\n          "infra"\n        ]');
});

test('kanban-board-builder deep-links params and flags WIP limits', async ({ page }) => {
  const params = new URLSearchParams({
    tasks: 'Backlog: polish docs\nIn Progress: migrate data\nReview: check API\nDone: ship changelog',
    columns: 'Backlog, In Progress, Review, Done',
    format: 'markdown',
    title: 'Sprint 12',
    default_column: 'Backlog',
    wip_limit: '0',
    sort_by: 'none',
    show_counts: 'false',
  });
  await page.goto(`/tools/kanban-board-builder/?${params.toString()}`);

  await expect(page.locator('#in-columns')).toHaveValue('Backlog, In Progress, Review, Done');
  await expect(page.locator('#in-format')).toHaveValue('markdown');
  await expect(page.locator('#in-title')).toHaveValue('Sprint 12');
  await expect(page.locator('#in-show_counts')).not.toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('# Sprint 12', { timeout: 15_000 });
  await expect(out).toContainText('## Backlog\n\n- [ ] polish docs');
  await expect(out).toContainText('## Review\n\n- [ ] check API');
  await expect(out).not.toContainText('(1)');

  await page.check('#in-show_counts');
  await page.fill('#in-wip_limit', '1');
  await expect(out).toContainText('## Backlog (1 / limit 1)', { timeout: 15_000 });
});

test('kanban-board-builder accepts exact WIP-limit cap boundary', async ({ page }) => {
  await page.goto('/tools/kanban-board-builder/');
  await page.fill(
    '#in-tasks',
    Array.from({ length: 100 }, (_, i) => `task ${i + 1}`).join('\n')
  );
  await page.fill('#in-columns', 'To Do');
  await page.fill('#in-wip_limit', '100');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('## To Do (100 / limit 100)', { timeout: 15_000 });
  await expect(out).not.toContainText('over WIP limit');
});
