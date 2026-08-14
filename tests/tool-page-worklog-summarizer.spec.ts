import { test, expect } from './fixtures';

const WORKLOG = '2024-01-15 09:00 @acme +dev writing the parser\n2024-01-15 10:30 @acme +review code review\n2024-01-15 12:00 lunch\n2024-01-15 13:00 @beta +dev bugfix\n2024-01-15 17:00 done';

async function runWasm(
  page: any,
  log: string = WORKLOG,
  groupBy = 'all',
  output = 'summary',
  units = 'hm',
  round = '0',
  maxEntry = '0',
  endTime = '',
  from = '',
  to = '',
  filter = '',
  defaultProject = '(untagged)',
  sort = 'duration',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/worklog-summarizer/gizza_ai_worklog_summarizer_web.js');
    await mod.default('/tools/worklog-summarizer/gizza_ai_worklog_summarizer_web_bg.wasm');
    return mod.run(
      args.log,
      args.groupBy,
      args.output,
      args.units,
      args.round,
      args.maxEntry,
      args.endTime,
      args.from,
      args.to,
      args.filter,
      args.defaultProject,
      args.sort,
    );
  }, { log, groupBy, output, units, round, maxEntry, endTime, from, to, filter, defaultProject, sort });
}

test('worklog-summarizer page computes a real worklog report from the form', async ({ page }) => {
  await page.goto('/tools/worklog-summarizer/');
  await page.fill('#in-log', WORKLOG);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Worklog summary', { timeout: 15_000 });
  await expect(out).toContainText('Entries: 4');
  await expect(out).toContainText('Tracked: 8h');
  await expect(out).toContainText('@beta');
  await expect(out).toContainText('50.0%');
});

test('worklog-summarizer deep link covers CSV, date filter, rounding, tag-list filter and time sort', async ({ page }) => {
  const params = new URLSearchParams({
    log: '2024-01-15 09:00 @acme draft\n2024-01-15 09:23 @acme review\n2024-01-15 10:01 @beta support\n2024-01-15 10:44 done',
    group_by: 'entry',
    output: 'csv',
    units: 'minutes',
    round: '15',
    max_entry: '0',
    from: '2024-01-15',
    to: '2024-01-15',
    filter: '@acme',
    default_project: 'personal',
    sort: 'time',
  });
  await page.goto(`/tools/worklog-summarizer/?${params.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('csv', { timeout: 15_000 });
  await expect(page.locator('#in-filter')).toHaveValue('@acme');
  await expect(page.locator('#tool-output')).toContainText('day,start,minutes,project,tags,entry', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('2024-01-15,09:00,30,@acme,@acme,draft');
  await expect(page.locator('#tool-output')).not.toContainText('@beta');
});

test('worklog-summarizer wasm covers enum values, cap boundary, open end time and CLI example', async ({ page }) => {
  await page.goto('/tools/worklog-summarizer/');

  const summary = await runWasm(page);
  expect(summary).toContain('Tracked: 8h');
  expect(summary).toContain('Time per project');

  const table = await runWasm(page, WORKLOG, 'tag', 'table', 'decimal');
  expect(table).toContain('tag\t+dev\t5.50\t68.8\t2');

  const csv = await runWasm(page, WORKLOG, 'day', 'csv', 'minutes');
  expect(csv).toContain('day,2024-01-15,480,100.0,4');

  const json = await runWasm(page, '09:00 @ops standup\n09:45 @ops incident review\n13:15 +focus write notes', 'project', 'json', 'decimal', '0', '240', '17:30', '', '', '', 'personal', 'name');
  expect(json).toContain('"total_minutes": 495');
  expect(json).toContain('"name": "+focus", "minutes": 240');
  expect(json).toContain('"name": "@ops", "minutes": 255');

  const entry = await runWasm(page, WORKLOG, 'entry', 'summary', 'hm', '0', '0', '', '', '', '', '(untagged)', 'time');
  expect(entry).toContain('2024-01-15 09:00');
  expect(entry).toContain('writing the parser');

  await expect(runWasm(page, '', 'all')).rejects.toThrow(/worklog is empty/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool worklog-summarizer');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

