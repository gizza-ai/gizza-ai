import { test, expect } from './fixtures';

async function runWasm(
  page,
  tasks: string,
  start = '2026-08-08',
  count = '5',
  defaultRec = '',
  skipWeekends = 'false',
  format = 'text',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/recurring-task-expander/gizza_ai_recurring_task_expander_web.js');
    await mod.default('/tools/recurring-task-expander/gizza_ai_recurring_task_expander_web_bg.wasm');
    return mod.run(args.tasks, args.start, args.count, args.defaultRec, args.skipWeekends, args.format);
  }, { tasks, start, count, defaultRec, skipWeekends, format });
}

const RENT_3 = `Pay rent due:2026-09-01
Pay rent due:2026-10-01
Pay rent due:2026-11-01`;

const GYM_MD = `- [ ] Gym — due 2026-08-10 (Mon)
- [ ] Gym — due 2026-08-13 (Thu)
- [ ] Gym — due 2026-08-17 (Mon)
- [ ] Gym — due 2026-08-20 (Thu)`;

const BACKUP_CSV = `task,recurrence,instance,date,weekday
Check backups,+2b,1,2026-08-13,Thursday
Check backups,+2b,2,2026-08-17,Monday
Check backups,+2b,3,2026-08-19,Wednesday
Check backups,+2b,4,2026-08-21,Friday
Check backups,+2b,5,2026-08-25,Tuesday`;

test('recurring-task-expander wasm expands strict monthly tasks exactly', async ({ page }) => {
  await page.goto('/tools/recurring-task-expander/');
  await page.waitForSelector('#in-tasks');

  await expect(runWasm(page, 'Pay rent due:2026-09-01 rec:+1m', '2026-08-08', '3')).resolves.toBe(RENT_3);
});

test('recurring-task-expander wasm covers weekday patterns and output formats', async ({ page }) => {
  await page.goto('/tools/recurring-task-expander/');
  await page.waitForSelector('#in-tasks');

  await expect(runWasm(page, 'Gym rec:mon,thu', '2026-08-08', '4', '', 'false', 'markdown')).resolves.toBe(GYM_MD);
  await expect(runWasm(page, 'Check backups due:2026-08-13 rec:+2b', '2026-08-13', '5', '', 'true', 'csv')).resolves.toBe(BACKUP_CSV);
  await expect(runWasm(page, 't rec:1d', '2026-08-08', '101')).rejects.toThrow(/count must be between 1 and 100/);
});

test('recurring-task-expander page renders exact output and honors non-default checkbox', async ({ page }) => {
  await page.goto('/tools/recurring-task-expander/');
  await page.fill('#in-tasks', 'Check email rec:+1d');
  await page.fill('#in-start', '2026-08-14');
  await page.fill('#in-count', '4');
  await page.check('#in-skip_weekends');

  await expect(page.locator('#tool-output')).toHaveText(`Check email due:2026-08-14
Check email due:2026-08-17
Check email due:2026-08-18
Check email due:2026-08-19`, { timeout: 15_000 });
});

test('recurring-task-expander deep-link prefills fields and emits JSON', async ({ page }) => {
  const params = new URLSearchParams({
    tasks: 'Water plants\nPay rent due:2026-09-01 rec:+1m',
    start: '2026-08-08',
    count: '2',
    default_rec: '1w',
    skip_weekends: 'false',
    format: 'json',
  });

  await page.goto(`/tools/recurring-task-expander/?${params.toString()}`);
  await expect(page.locator('#in-tasks')).toHaveValue('Water plants\nPay rent due:2026-09-01 rec:+1m', { timeout: 15_000 });
  await expect(page.locator('#in-start')).toHaveValue('2026-08-08');
  await expect(page.locator('#in-count')).toHaveValue('2');
  await expect(page.locator('#in-default_rec')).toHaveValue('1w');
  await expect(page.locator('#in-skip_weekends')).not.toBeChecked();
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"description": "Water plants"', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('"date": "2026-08-15"');
  await expect(page.locator('#tool-output')).toContainText('"recurrence": "+1m"');
});
