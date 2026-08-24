import { test, expect } from './fixtures';

const JIRA_CSV = `Issue key,Summary,Issue Type,Status,Priority,Assignee,Story Points,Created,Resolved,Sprint
GIZ-1,Login page,Story,Done,High,ana,3,05/Jan/24 9:00 AM,08/Jan/24 5:00 PM,Sprint 1
GIZ-2,Fix crash,Bug,Done,High,bo,2,05/Jan/24 9:00 AM,06/Jan/24 9:00 AM,Sprint 1
GIZ-3,Docs,Task,In Progress,Low,ana,1,06/Jan/24 9:00 AM,,Sprint 1
`;

const LINEAR_CSV = `ID,Title,Status,Assignee,Estimate,Created,Started,Completed,Cycle Name
ENG-1,Ship it,Done,ana,3,2024-01-05T09:00:00Z,2024-01-06T09:00:00Z,2024-01-08T09:00:00Z,Cycle 4
ENG-2,Bug,Done,bo,1,2024-01-05T09:00:00Z,2024-01-05T09:00:00Z,2024-01-07T09:00:00Z,Cycle 4
ENG-3,Idea,Backlog,,2,2024-01-06T09:00:00Z,,,
`;

async function outputText(page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('issue-export-reporter page renders a real Jira summary', async ({ page }) => {
  await page.goto('/tools/issue-export-reporter/');
  await page.fill('#in-data', JIRA_CSV);
  await page.selectOption('#in-report', 'summary');
  await page.selectOption('#in-format', 'text');
  await page.selectOption('#in-delimiter', 'auto');
  await page.selectOption('#in-group_by', 'none');
  await page.selectOption('#in-period', 'auto');
  await page.selectOption('#in-unit', 'days');
  await page.fill('#in-percentiles', '50,85,95');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Overview', { timeout: 20_000 });
  await expect(out).toContainText('Completed         2');
  await expect(out).toContainText('Completion rate   66.7%');
  await expect(out).toContainText('Velocity by sprint');
  await expect(out).toContainText('Sprint 1          2       5');
  expect(await outputText(page)).toContain('No start column');
});

test('issue-export-reporter page covers enums, checkbox and deep-linked params', async ({ page }) => {
  const data = encodeURIComponent(LINEAR_CSV);
  await page.goto(
    `/tools/issue-export-reporter/?data=${data}&report=cycle_time&format=json&delimiter=comma&group_by=assignee&period=week&unit=hours&business_days=true&percentiles=30%2C50%2C70`,
  );

  await expect(page.locator('#in-report')).toHaveValue('cycle_time');
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#in-delimiter')).toHaveValue('comma');
  await expect(page.locator('#in-group_by')).toHaveValue('assignee');
  await expect(page.locator('#in-period')).toHaveValue('week');
  await expect(page.locator('#in-unit')).toHaveValue('hours');
  await expect(page.locator('#in-business_days')).toBeChecked();
  await expect(page.locator('#in-percentiles')).toHaveValue('30,50,70');

  await expect(page.locator('#tool-output')).toContainText('"report": "cycle_time"', {
    timeout: 20_000,
  });
  const linked = JSON.parse(await outputText(page));
  expect(linked.report).toBe('cycle_time');
  expect(linked.source).toBe('linear');
  expect(linked.sections.some((section: { title: string }) => section.title.includes('Lead time'))).toBe(
    true,
  );
  expect(linked.sections.some((section: { title: string }) => section.title.includes('Cycle time'))).toBe(
    true,
  );
});

test('issue-export-reporter page supports CSV velocity output and custom columns', async ({ page }) => {
  const renamed = `Key,State,Estimate,Opened,Done At,Team Sprint
ONE-1,Resolved,5,2024-01-02,2024-01-05,Sprint A
ONE-2,Open,3,2024-01-03,,Sprint A
`;

  await page.goto('/tools/issue-export-reporter/');
  await page.fill('#in-data', renamed);
  await page.selectOption('#in-report', 'velocity');
  await page.selectOption('#in-format', 'csv');
  await page.fill(
    '#in-columns',
    'key=Key,status=State,points=Estimate,created=Opened,resolved=Done At,sprint=Team Sprint',
  );
  await page.fill('#in-done_statuses', 'resolved');
  await page.fill('#in-cancelled_statuses', '');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Sprint,Completed,Points', { timeout: 20_000 });
  await expect(out).toContainText('Sprint A,1,5');
});
