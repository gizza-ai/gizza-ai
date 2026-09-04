import { test, expect } from './fixtures';

// /tools/confusion-matrix/ builds a local classification report from labels/counts.
test('confusion-matrix renders binary metrics from an aggregated table', async ({ page }) => {
  await page.goto('/tools/confusion-matrix/');
  await page.fill('#in-actual', 'actual,predicted,count\nspam,spam,180\nspam,ham,20\nham,spam,40\nham,ham,760');
  await page.fill('#in-positive_label', 'spam');
  await page.selectOption('#in-input_format', 'table');
  await page.selectOption('#in-format', 'text');
  await page.check('#in-percent');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Confusion matrix (rows = actual, columns = predicted)', { timeout: 15000 });
  await expect(out).toContainText('accuracy                         94.0000%');
  await expect(out).toContainText('precision (PPV)                  81.8182%');
  await expect(out).toContainText('recall (sensitivity, TPR)        90.0000%');
  await expect(out).toContainText('specificity (TNR)                95.0000%');
});

test('confusion-matrix honors deep-linked matrix parameters', async ({ page }) => {
  const params = new URLSearchParams({
    actual: ',cat,dog\ncat,5,1\ndog,2,7',
    input_format: 'matrix',
    separator: 'comma',
    header: 'yes',
    positive_label: 'dog',
    format: 'json',
    beta: '2',
    decimals: '4',
  });
  await page.goto(`/tools/confusion-matrix/?${params.toString()}`);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"labels": ["cat", "dog"]', { timeout: 15000 });
  await expect(out).toContainText('"matrix": [[5, 1], [2, 7]]');
  await expect(out).toContainText('"accuracy": 0.8000');
  await expect(out).toContainText('"positive_label": "dog"');
});
