import { test, expect } from './fixtures';

const tool = '/tools/likert-summary/';
const responsesCsv = 'Ease of use,Support,Value for money\n5,2,4\n4,3,4\n5,1,3';
const expectedDefault = `Likert summary — 3 items, 3 respondents
Scale: 5-point agreement (1 = Strongly disagree … 5 = Strongly agree)
Box size: 2 categories each end. Missing: exclude.

Item                 n   miss     mean       sd   median    mode   Bottom 2    Neutral      Top 2
-------------------------------------------------------------------------------------------------
Ease of use          3      0     4.67     0.58     5.00       5       0.0%       0.0%     100.0%
Support              3      0     2.00     1.00     2.00   1,2,3      66.7%      33.3%       0.0%
Value for money      3      0     3.67     0.58     4.00       4       0.0%      33.3%      66.7%

Overall mean of item means: 3.44 (9 valid answers, 0 missing)
Floor/ceiling effects (>= 15% at an end category):
  ceiling: Ease of use — 66.7% chose the highest category (Strongly agree)
  floor: Support — 33.3% chose the lowest category (Strongly disagree)

Distribution (count and % of that item's valid answers)

Ease of use (n = 3)
  1 Strongly disagree      0     0.0%
  2 Disagree               0     0.0%
  3 Neutral                0     0.0%
  4 Agree                  1    33.3%
  5 Strongly agree         2    66.7%

Support (n = 3)
  1 Strongly disagree      1    33.3%
  2 Disagree               1    33.3%
  3 Neutral                1    33.3%
  4 Agree                  0     0.0%
  5 Strongly agree         0     0.0%

Value for money (n = 3)
  1 Strongly disagree      0     0.0%
  2 Disagree               0     0.0%
  3 Neutral                1    33.3%
  4 Agree                  2    66.7%
  5 Strongly agree         0     0.0%

Stacked bars (40 chars = 100% of that item's valid answers)
  Ease of use      4444444444444555555555555555555555555555
  Support          1111111111111122222222222223333333333333
  Value for money  3333333333333444444444444444444444444444
  Key: 1=Strongly disagree  2=Disagree  3=Neutral  4=Agree  5=Strongly agree`;
const countsCsv = 'Item,Never,Rarely,Sometimes,Often,Always\nWeekly standup,1,2,5,8,4\nSprint review,0,1,3,9,7';
const expectedCounts = `Likert summary — 2 items, 20 respondents
Scale: 5-point frequency (1 = Never … 5 = Always)
Box size: 2 categories each end. Missing: exclude.

Item                n   miss     mean       sd   median    mode   Bottom 2    Neutral      Top 2
------------------------------------------------------------------------------------------------
Weekly standup     20      0     3.60     1.10     4.00       4      15.0%      25.0%      60.0%
Sprint review      20      0     4.10     0.85     4.00       4       5.0%      15.0%      80.0%`;

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  data = responsesCsv,
  input = 'responses',
  items = '',
  points = '5',
  scale = 'agreement',
  labels = '',
  reverse = '',
  boxSize = '2',
  missing = 'exclude',
  sort = 'input',
  decimals = '2',
  chart = 'true',
  diverging = 'false',
  alpha = 'false',
  delimiter = ',',
): Promise<string> {
  return await page.evaluate(
    async ({ data, input, items, points, scale, labels, reverse, boxSize, missing, sort, decimals, chart, diverging, alpha, delimiter }) => {
      const mod = await import('/tools/likert-summary/gizza_ai_likert_summary_web.js');
      await mod.default('/tools/likert-summary/gizza_ai_likert_summary_web_bg.wasm');
      return mod.run(data, input, items, points, scale, labels, reverse, boxSize, missing, sort, decimals, chart, diverging, alpha, delimiter);
    },
    { data, input, items, points, scale, labels, reverse, boxSize, missing, sort, decimals, chart, diverging, alpha, delimiter },
  );
}

test('likert-summary page renders the default response CSV exactly', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-data'), responsesCsv);
  await expect(page.locator('#tool-output')).toHaveText(expectedDefault, { timeout: 15_000 });
});

test('likert-summary deep link pre-fills counts mode and diverging bars', async ({ page }) => {
  const qs = new URLSearchParams({
    data: countsCsv,
    input: 'counts',
    points: '5',
    scale: 'frequency',
    box_size: '2',
    chart: 'true',
    diverging: 'true',
  });
  await page.goto(`${tool}?${qs.toString()}`);
  await expect(page.locator('#in-data')).toHaveValue(countsCsv, { timeout: 15_000 });
  await expect(page.locator('#in-input')).toHaveValue('counts');
  await expect(page.locator('#in-scale')).toHaveValue('frequency');
  await expect(page.locator('#in-diverging')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText(expectedCounts, { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('Diverging stacked bars');
});

test('likert-summary wasm covers advertised choices, checkboxes, caps and errors', async ({ page }) => {
  await page.goto(tool);

  const selectedItems = await runWasm(page, responsesCsv, 'responses', 'Ease of use,Support', '5', 'agreement', '', 'Support', '1', 'listwise', 'mean-desc', '1', 'false', 'false', 'true');
  expect(selectedItems).toContain('Likert summary — 2 items, 3 respondents');
  expect(selectedItems).toContain("Cronbach's alpha:");
  expect(selectedItems).toContain('Support          3      0      4.0');
  expect(selectedItems).not.toContain('Stacked bars');

  const custom = await runWasm(page, 'Q\nlo\nmid\nhi\n', 'responses', '', '3', 'custom', 'lo,mid,hi', '', '1', 'exclude', 'input', '0', 'true');
  expect(custom).toContain('Scale: 3-point custom (1 = lo … 3 = hi)');
  expect(custom).toContain('Overall mean of item means: 2 (3 valid answers, 0 missing)');

  const elevenPointBoundary = await runWasm(page, 'Q\n1\n11\n6\n', 'responses', '', '11', 'numeric', '', '', '5', 'exclude', 'input', '6', 'true');
  expect(elevenPointBoundary).toContain('Scale: 11-point numeric');
  expect(elevenPointBoundary).toContain('5.000000');
  expect(elevenPointBoundary).toContain('a=10');

  const semicolonCounts = await runWasm(page, 'Item;Low;Mid-low;Mid-high;High\nQ1;0;1;2;3', 'counts', '', '4', 'numeric', '', '', '1', 'exclude', 'top-desc', '2', 'true', 'false', 'false', 'semicolon');
  expect(semicolonCounts).toContain('Likert summary — 1 item, 6 respondents');
  expect(semicolonCounts).toContain('3.33');

  await expect(runWasm(page, 'Q\n12\n', 'responses', '', '11', 'numeric')).rejects.toThrow(/outside the 1-11 scale/);
  await expect(runWasm(page, 'Q\nlo\n', 'responses', '', '3', 'custom', 'low,high', '', '1')).rejects.toThrow(/labels lists 2 value/);
  await expect(runWasm(page, responsesCsv, 'responses', '', '5', 'agreement', '', '', '3')).rejects.toThrow(/box_size must be between 1 and 2/);
});

test('likert-summary ships competitor-derived example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await expect(page.locator('.tool-example-chip')).toContainText([
    '5-point agreement',
    'Reverse-score an item + alpha',
    'Counts + diverging bars',
    '7-point satisfaction',
  ]);
});
