import { test, expect } from './fixtures';

const tool = '/tools/focus-picker/';
const TASKS = 'Fix login redirect !p1 due:tomorrow est:90m\nWrite release notes !p2 due:+3d est:2h\nRefactor settings page !p3 est:1d\nBook retro room !p4 due:friday est:15m';

const EXPECTED = `Focus on: Fix login redirect
Why: p1 priority, due 2026-08-22 (tomorrow), ~1.5 h effort — highest balanced score (79.9) of 4 tasks.

Full ranking
  1.   79.9  Fix login redirect — p1 · due 2026-08-22 (tomorrow) · 1.5 h
  2.   64.5  Write release notes — p2 · due 2026-08-24 (in 3 days) · 2 h
  3.   61.8  Book retro room — p4 · due 2026-08-21 (today) · 0.25 h
  4.   32.5  Refactor settings page — p3 · no due date · 8 h

4 tasks · 11.75 h total effort · 0 overdue · method balanced (0.45 x priority + 0.35 x urgency + 0.20 x effort-ease, scaled to 100)`;

async function runWasm(
  page: import('@playwright/test').Page,
  params: {
    tasks?: string;
    method?: string;
    today?: string;
    default_priority?: string;
    default_effort?: number;
    overdue_boost?: string;
    format?: string;
    show_ranking?: string;
  } = {},
) {
  const p = {
    tasks: TASKS,
    method: 'balanced',
    today: '2026-08-21',
    default_priority: 'p3',
    default_effort: 2,
    overdue_boost: 'true',
    format: 'text',
    show_ranking: 'true',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/focus-picker/gizza_ai_focus_picker_web.js');
    await mod.default('/tools/focus-picker/gizza_ai_focus_picker_web_bg.wasm');
    return mod.run(
      args.tasks,
      args.method,
      args.today,
      args.default_priority,
      args.default_effort,
      args.overdue_boost,
      args.format,
      args.show_ranking,
    );
  }, p);
}

test('focus-picker page renders the exact balanced focus recommendation', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-tasks', TASKS);
  await page.selectOption('#in-method', 'balanced');
  await page.fill('#in-today', '2026-08-21');
  await page.selectOption('#in-default_priority', 'p3');
  await page.fill('#in-default_effort', '2');
  await page.check('#in-overdue_boost');
  await page.selectOption('#in-format', 'text');
  await page.check('#in-show_ranking');

  await expect(page.locator('#tool-output')).toHaveText(EXPECTED, { timeout: 15_000 });
});

test('focus-picker deep-link renders markdown deadline mode', async ({ page }) => {
  const tasks = 'Submit tax documents !p2 due:today est:3h\nRenew parking permit !p3 due:tomorrow est:20m\nUpdate portfolio !p1 due:+10d est:5h';
  const qs = new URLSearchParams({
    tasks,
    method: 'deadline',
    today: '2026-08-21',
    default_priority: 'p3',
    default_effort: '2',
    overdue_boost: 'true',
    format: 'markdown',
    show_ranking: 'true',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-tasks')).toHaveValue(tasks, { timeout: 15_000 });
  await expect(page.locator('#in-method')).toHaveValue('deadline');
  await expect(page.locator('#in-format')).toHaveValue('markdown');
  await expect(page.locator('#tool-output')).toContainText('**Focus on:** Submit tax documents', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('| # | Score | Task | Priority | Due | Effort |');
});

test('focus-picker wasm covers methods, outputs, booleans and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-tasks');

  await expect(runWasm(page)).resolves.toBe(`${EXPECTED}\n`);
  await expect(runWasm(page, { method: 'quick-wins' })).resolves.toContain('highest quick-wins score');
  await expect(runWasm(page, { method: 'wsjf' })).resolves.toContain('highest WSJF');
  await expect(runWasm(page, { method: 'eisenhower' })).resolves.toContain('Eisenhower quadrant');
  await expect(runWasm(page, { format: 'markdown' })).resolves.toContain('| # | Score | Task | Priority | Due | Effort |');
  const json = JSON.parse(await runWasm(page, { format: 'json' }));
  expect(json.pick.task).toBe('Fix login redirect');
  expect(json.ranked).toHaveLength(4);
  await expect(runWasm(page, { show_ranking: 'false' })).resolves.not.toContain('Full ranking');

  const overdue = await runWasm(page, {
    tasks: 'Old invoice !p4 due:yesterday est:15m\nStrategic plan !p1 due:+5d est:3h',
    overdue_boost: 'true',
  });
  expect(overdue).toContain('Focus on: Old invoice');

  await expect(runWasm(page, { method: 'mystery' })).rejects.toThrow(/unknown method/);
  await expect(runWasm(page, { today: '21\/08\/2026' })).rejects.toThrow(/YYYY-MM-DD/);
  await expect(runWasm(page, { default_effort: 10001 })).rejects.toThrow(/default_effort/);
});

test('focus-picker ships example chips and a generated CLI example', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Launch triage',
    'Deadline mode',
    'WSJF backlog',
    'Eisenhower clean-up',
  ]);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool focus-picker');
  expect(cli).toContain('Fix login redirect');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
