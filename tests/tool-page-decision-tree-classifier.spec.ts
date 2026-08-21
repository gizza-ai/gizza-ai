import { test, expect } from './fixtures';

const FRUIT = `color,size,ripe
red,small,yes
red,large,yes
green,small,no
green,large,no`;

const NUMERIC = `hours,score,pass
1,10,no
2,20,no
3,30,no
4,40,no
6,60,yes
7,70,yes
8,80,yes
9,90,yes`;

const OUTLOOK = `outlook,play
sunny,no
sunny,no
overcast,yes
overcast,yes
rain,yes
rain,no`;

async function setTextarea(page: any, selector: string, value: string) {
  await page.locator(selector).evaluate((el: HTMLTextAreaElement, v: string) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('decision-tree-classifier renders the exact rules report', async ({ page }) => {
  await page.goto('/tools/decision-tree-classifier/');
  await setTextarea(page, '#in-data', FRUIT);
  await page.fill('#in-target', 'ripe');
  await page.selectOption('#in-criterion', 'gini');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText(`Decision tree classifier
Criterion: gini (CART)
Splits: binary
Target: ripe (2 classes: no, yes)
Features (2): color, size
Rows: 4 used
Tree size: depth 1, 2 leaves, 3 nodes

Tree:
├─ color = green → no  [n=2, 100.0%]
└─ color != green → yes  [n=2, 100.0%]

Rules:
1. IF color = green THEN ripe = no  [n=2, 100.0%]
2. IF color != green THEN ripe = yes  [n=2, 100.0%]

Feature importance:
  color  1
  size   0

Training accuracy: 1 (4/4 correct)

Confusion matrix (rows = actual, columns = predicted):
       no yes
  no    2   0
  yes   0   2`, { timeout: 15_000 });
});

test('decision-tree-classifier deep link covers entropy, numeric thresholds, hold-out and JSON', async ({ page }) => {
  const params = new URLSearchParams({
    data: NUMERIC,
    target: '3',
    features: '1,2',
    criterion: 'entropy',
    splits: 'binary',
    max_depth: '3',
    min_samples_split: '2',
    min_samples_leaf: '1',
    min_gain: '0',
    class_weight: 'none',
    test_split: '0.25',
    seed: '7',
    header: 'yes',
    decimals: '3',
    format: 'json',
  });
  await page.goto(`/tools/decision-tree-classifier/?${params.toString()}`);

  await expect(page.locator('#in-target')).toHaveValue('3', { timeout: 15_000 });
  await expect(page.locator('#in-features')).toHaveValue('1,2');
  await expect(page.locator('#in-criterion')).toHaveValue('entropy');
  await expect(page.locator('#in-splits')).toHaveValue('binary');
  await expect(page.locator('#in-max_depth')).toHaveValue('3');
  await expect(page.locator('#in-test_split')).toHaveValue('0.25');
  await expect(page.locator('#in-seed')).toHaveValue('7');
  await expect(page.locator('#in-header')).toHaveValue('yes');
  await expect(page.locator('#in-format')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"criterion":"entropy"', { timeout: 15_000 });
  await expect(out).toContainText('"target":"pass"');
  await expect(out).toContainText('"classes":["no","yes"]');
  await expect(out).toContainText('"type":"numeric"');
  await expect(out).toContainText('"test":');
  await expect(out).toContainText('"rules":[');
  await expect(out).toContainText('"tree":');
});

test('decision-tree-classifier covers gain ratio, multiway splits and CSV output', async ({ page }) => {
  const params = new URLSearchParams({
    data: OUTLOOK,
    target: 'play',
    criterion: 'gain_ratio',
    splits: 'multiway',
    format: 'csv',
  });
  await page.goto(`/tools/decision-tree-classifier/?${params.toString()}`);

  await expect(page.locator('#in-criterion')).toHaveValue('gain_ratio', { timeout: 15_000 });
  await expect(page.locator('#in-splits')).toHaveValue('multiway');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('section,name,value', { timeout: 15_000 });
  await expect(out).toContainText('model,criterion,gain_ratio');
  await expect(out).toContainText('model,splits,multiway');
  await expect(out).toContainText('outlook = overcast');
  await expect(out).toContainText('importance,outlook,1');
  await expect(out).toContainText('accuracy,train,');
  // Multiway must not fall back to the binary one-vs-rest condition.
  await expect(out).not.toContainText('outlook !=');
});

test('decision-tree-classifier covers Graphviz DOT output', async ({ page }) => {
  const params = new URLSearchParams({ data: FRUIT, target: 'ripe', format: 'dot' });
  await page.goto(`/tools/decision-tree-classifier/?${params.toString()}`);

  await expect(page.locator('#in-format')).toHaveValue('dot', { timeout: 15_000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('digraph DecisionTree {', { timeout: 15_000 });
  await expect(out).toContainText('n0 -> n1');
  await expect(out).toContainText('color = green');
});

test('decision-tree-classifier predicts pasted rows and honours balanced weights', async ({ page }) => {
  await page.goto('/tools/decision-tree-classifier/');
  await setTextarea(page, '#in-data', FRUIT);
  await page.fill('#in-target', 'ripe');
  await setTextarea(page, '#in-predict', 'color,size\ngreen,large\nred,small');
  await page.selectOption('#in-class_weight', 'balanced');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Class weight: balanced', { timeout: 15_000 });
  await expect(out).toContainText('Predictions (2 rows):');
  await expect(out).toContainText('1. green, large → no  [100.0% confidence, rule 1, n=2]');
  await expect(out).toContainText('2. red, small → yes  [100.0% confidence, rule 2, n=2]');
});

test('decision-tree-classifier honours the depth cap boundary and pre-pruning', async ({ page }) => {
  const params = new URLSearchParams({
    data: NUMERIC,
    target: 'pass',
    max_depth: '20',
    min_samples_split: '100',
    decimals: '0',
  });
  await page.goto(`/tools/decision-tree-classifier/?${params.toString()}`);

  await expect(page.locator('#in-max_depth')).toHaveValue('20', { timeout: 15_000 });
  await expect(page.locator('#in-min_samples_split')).toHaveValue('100');
  const out = page.locator('#tool-output');
  // min_samples_split above the row count blocks every split: a bare stump.
  await expect(out).toContainText('Tree size: depth 0, 1 leaves, 1 nodes', { timeout: 15_000 });
  await expect(out).toContainText('IF always THEN pass =');
});

test('decision-tree-classifier reports a usable error for a bad column', async ({ page }) => {
  const params = new URLSearchParams({ data: FRUIT, target: 'nope' });
  await page.goto(`/tools/decision-tree-classifier/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText("target column 'nope' not found", { timeout: 15_000 });
});

test('decision-tree-classifier generated CLI example is generic and parseable', async ({ page }) => {
  await page.goto('/tools/decision-tree-classifier/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool decision-tree-classifier');
  expect(cli).toContain('color,size,ripe');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
