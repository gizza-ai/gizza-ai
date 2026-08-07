import { test, expect } from './fixtures';

const TRAIN = 'spam,win a free prize now\nspam,free money click here\nham,meeting at ten tomorrow\nham,lunch with the team';

const SPAM_REPORT = `Prediction: spam
Confidence: 91.4%

Class scores
  class  probability   log score
  spam         91.4%     -7.8649
  ham           8.6%    -10.2273

Top tokens for "spam" over "ham"
  free     +1.0578
  money    +0.6523
  now      +0.6523

Input
  tokens            5
  seen in training  3

Model
  algorithm          multinomial naive Bayes
  smoothing alpha    1
  n-grams            1..1
  lowercase          on
  stopwords removed  off
  min token count    1
  class priors       empirical

Training data
  separator   comma (auto-detected)
  examples    4
  classes     2
  vocabulary  16 tokens
  ham         2 examples, 8 tokens
  spam        2 examples, 9 tokens

Notes
  - 2 of 5 input tokens were never seen in training and were ignored.`;

const COMPLEMENT_JSON = `{
  "classes": [
    {
      "label": "good",
      "probability": 0.964286,
      "score": 11.613603
    },
    {
      "label": "bad",
      "probability": 0.035714,
      "score": 8.317766
    }
  ],
  "confidence": 0.964286,
  "model": {
    "algorithm": "complement",
    "alpha": 0.5,
    "lowercase": true,
    "min_count": 1,
    "ngram_max": 2,
    "priors": "uniform",
    "remove_stopwords": true
  },
  "notes": [
    "2 of 5 input tokens were never seen in training and were ignored.",
    "Complement naive Bayes scores from complement-class weights and ignores class priors; the percentages are normalised scores, not calibrated probabilities."
  ],
  "prediction": "good",
  "tokens": 5,
  "tokens_seen_in_training": 3,
  "training": {
    "classes": 2,
    "examples": 4,
    "per_class": [
      {
        "examples": 2,
        "label": "bad",
        "tokens": 12
      },
      {
        "examples": 2,
        "label": "good",
        "tokens": 12
      }
    ],
    "separator": "comma",
    "separator_auto_detected": true,
    "vocabulary": 24
  }
}`;

test('naive-bayes-text-classifier predicts spam with exact report output', async ({ page }) => {
  await page.goto('/tools/naive-bayes-text-classifier/');
  await page.fill('#in-training_data', TRAIN);
  await page.fill('#in-text', 'claim your free money now');

  await expect(page.locator('#tool-output')).toHaveText(SPAM_REPORT, { timeout: 15_000 });
});

test('naive-bayes-text-classifier deep-link runs complement JSON with non-default controls', async ({ page }) => {
  const params = new URLSearchParams({
    training_data: 'good,great helpful friendly service\ngood,love the fast support\nbad,terrible broken slow response\nbad,refund because product failed',
    text: 'fast friendly support',
    separator: 'auto',
    input_mode: 'single',
    model: 'complement',
    alpha: '0.5',
    ngram_max: '2',
    lowercase: 'true',
    remove_stopwords: 'true',
    min_count: '1',
    priors: 'uniform',
    top_k: '0',
    explain: 'false',
    output: 'json',
  });

  await page.goto(`/tools/naive-bayes-text-classifier/?${params.toString()}`);
  await expect(page.locator('#in-model')).toHaveValue('complement');
  await expect(page.locator('#in-alpha')).toHaveValue('0.5');
  await expect(page.locator('#in-ngram_max')).toHaveValue('2');
  await expect(page.locator('#in-remove_stopwords')).toBeChecked();
  await expect(page.locator('#in-explain')).not.toBeChecked();
  await expect(page.locator('#in-priors')).toHaveValue('uniform');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toHaveText(COMPLEMENT_JSON, { timeout: 15_000 });
});
