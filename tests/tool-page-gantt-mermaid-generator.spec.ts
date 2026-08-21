import { test, expect } from './fixtures';

const tool = '/tools/gantt-mermaid-generator/';
const TASKS = 'section Design\nWireframes | 2026-03-02 | 5d | done\nVisual design | after Wireframes | 1w | active\nDesign sign-off | after Visual design | | milestone';

const EXPECTED = `gantt
    title Q2 launch plan
    dateFormat YYYY-MM-DD
    axisFormat %b %d
    tickInterval 1week
    excludes weekends
    section Design
        Wireframes      :done, wireframes, 2026-03-02, 5d
        Visual design   :active, visual_design, after wireframes, 1w
        Design sign-off :milestone, design_sign_off, after visual_design, 0d`;

async function runWasm(
  page: import('@playwright/test').Page,
  params: {
    tasks?: string;
    title?: string;
    delimiter?: string;
    date_format?: string;
    axis_format?: string;
    tick_interval?: string;
    exclude_weekends?: string;
    weekend?: string;
    excludes?: string;
    today_marker?: string;
    compact?: string;
    fence?: string;
  } = {},
) {
  const p = {
    tasks: TASKS,
    title: 'Q2 launch plan',
    delimiter: 'auto',
    date_format: 'YYYY-MM-DD',
    axis_format: '%b %d',
    tick_interval: '1week',
    exclude_weekends: 'true',
    weekend: 'saturday',
    excludes: '',
    today_marker: 'true',
    compact: 'false',
    fence: 'false',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/gantt-mermaid-generator/gizza_ai_gantt_mermaid_generator_web.js');
    await mod.default('/tools/gantt-mermaid-generator/gizza_ai_gantt_mermaid_generator_web_bg.wasm');
    return mod.run(
      args.tasks,
      args.title,
      args.delimiter,
      args.date_format,
      args.axis_format,
      args.tick_interval,
      args.exclude_weekends,
      args.weekend,
      args.excludes,
      args.today_marker,
      args.compact,
      args.fence,
    );
  }, p);
}

test('gantt-mermaid-generator page renders exact Mermaid output', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-tasks', TASKS);
  await page.fill('#in-title', 'Q2 launch plan');
  await page.fill('#in-axis_format', '%b %d');
  await page.fill('#in-tick_interval', '1week');
  await page.check('#in-exclude_weekends');

  await expect(page.locator('#tool-output')).toHaveText(EXPECTED, { timeout: 15_000 });
});

test('gantt-mermaid-generator deep-link renders fenced day-first timeline', async ({ page }) => {
  const tasks = 'section Fit-out\nSurvey | 02/03/2026 | 3d | done\nElectrics | after Survey | 2w\nFurniture | after Electrics | 1w\nHandover | after Furniture | | milestone';
  const qs = new URLSearchParams({
    tasks,
    title: 'Office fit-out',
    delimiter: 'pipe',
    date_format: 'DD/MM/YYYY',
    axis_format: '%d/%m',
    tick_interval: '1week',
    exclude_weekends: 'true',
    weekend: 'friday',
    excludes: 'monday',
    today_marker: 'false',
    compact: 'true',
    fence: 'true',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-tasks')).toHaveValue(tasks, { timeout: 15_000 });
  await expect(page.locator('#in-date_format')).toHaveValue('DD/MM/YYYY');
  await expect(page.locator('#in-weekend')).toHaveValue('friday');
  await expect(page.locator('#in-exclude_weekends')).toBeChecked();
  await expect(page.locator('#in-today_marker')).not.toBeChecked();
  await expect(page.locator('#in-compact')).toBeChecked();
  await expect(page.locator('#in-fence')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('```mermaid', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('dateFormat DD/MM/YYYY');
  await expect(page.locator('#tool-output')).toContainText('weekend friday');
  await expect(page.locator('#tool-output')).toContainText('todayMarker off');
  await expect(page.locator('#tool-output')).toContainText('displayMode: compact');
  await expect(page.locator('#tool-output')).toContainText('Handover  :milestone, handover, after furniture, 0d');
});

test('gantt-mermaid-generator wasm covers advertised controls and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-tasks');

  await expect(runWasm(page)).resolves.toBe(EXPECTED);
  await expect(runWasm(page, { delimiter: 'pipe' })).resolves.toContain('Wireframes');
  await expect(runWasm(page, { weekend: 'friday', excludes: 'monday', today_marker: 'false' })).resolves.toContain('weekend friday');
  await expect(runWasm(page, { weekend: 'friday', excludes: 'monday', today_marker: 'false' })).resolves.toContain('todayMarker off');
  await expect(runWasm(page, { compact: 'true' })).resolves.toContain('displayMode: compact');
  await expect(runWasm(page, { fence: 'true' })).resolves.toMatch(/^```mermaid/);

  const csv = await runWasm(page, {
    tasks: 'Kick off,2026-03-02,1d,done\nDiscovery,after Kick off,2w\nPilot,after Discovery,3w,crit',
    delimiter: 'comma',
    title: 'Rollout programme',
    axis_format: '%b',
    tick_interval: '1month',
    exclude_weekends: 'false',
    today_marker: 'false',
    compact: 'true',
  });
  expect(csv).toContain('displayMode: compact');
  expect(csv).toContain('Rollout programme');
  expect(csv).toContain('Pilot     :crit, pilot, after discovery, 3w');

  await expect(runWasm(page, { date_format: 'bogus' })).rejects.toThrow(/unknown date_format/);
  await expect(runWasm(page, { tick_interval: 'week' })).rejects.toThrow(/invalid tick_interval/);
  await expect(runWasm(page, { tasks: 'Task | 03\/02\/2026 | 1d' })).rejects.toThrow(/YYYY-MM-DD/);
});

test('gantt-mermaid-generator ships example chips and a generated CLI example', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Design sprint',
    'Two-week sprint',
    'CSV columns',
    'Day/month dates',
  ]);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool gantt-mermaid-generator');
  expect(cli).toContain('section Design');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
