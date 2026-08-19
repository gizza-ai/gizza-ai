import { test, expect } from './fixtures';

const sshLogs = [
  'Jan 12 03:04:05 web1 sshd[2311]: Failed password for root from 10.0.0.1 port 51234 ssh2',
  'Jan 12 03:04:07 web1 sshd[2312]: Failed password for admin from 10.0.0.2 port 51999 ssh2',
  'Jan 12 03:04:09 web1 sshd[2313]: Failed password for root from 10.0.0.3 port 52001 ssh2',
  'Jan 12 03:05:00 web1 sshd[2400]: Accepted publickey for deploy from 10.0.0.9 port 60001 ssh2',
].join('\n');

async function fillMiner(page, opts: {
  logs?: string;
  format?: string;
  similarity?: string;
  depth?: string;
  maxPatterns?: string;
  minCount?: string;
  mask?: string;
  extraDelimiters?: string;
  skipTokens?: string;
}) {
  if (opts.logs !== undefined) await page.fill('#in-logs', opts.logs);
  if (opts.format !== undefined) await page.selectOption('#in-format', opts.format);
  if (opts.similarity !== undefined) await page.fill('#in-similarity', opts.similarity);
  if (opts.depth !== undefined) await page.fill('#in-depth', opts.depth);
  if (opts.maxPatterns !== undefined) await page.fill('#in-max_patterns', opts.maxPatterns);
  if (opts.minCount !== undefined) await page.fill('#in-min_count', opts.minCount);
  if (opts.mask !== undefined) await page.selectOption('#in-mask', opts.mask);
  if (opts.extraDelimiters !== undefined) await page.fill('#in-extra_delimiters', opts.extraDelimiters);
  if (opts.skipTokens !== undefined) await page.fill('#in-skip_tokens', opts.skipTokens);
}

test('log-pattern-miner clusters SSH failures into ranked templates', async ({ page }) => {
  await page.goto('/tools/log-pattern-miner/');
  await fillMiner(page, { logs: sshLogs, format: 'table' });

  await expect(page.locator('#tool-output')).toHaveText(
    'count\tpercent\tfirst\tlast\ttemplate\n' +
      '3\t75\t1\t3\tJan <NUM> <TIME> web1 sshd[<NUM>]: Failed password for <*> from <IP> port <NUM> ssh2\n' +
      '1\t25\t4\t4\tJan <NUM> <TIME> web1 sshd[<NUM>]: Accepted publickey for deploy from <IP> port <NUM> ssh2',
    { timeout: 20000 },
  );
});

test('log-pattern-miner deep link applies format, mask, delimiter and min-count controls', async ({ page }) => {
  const qs =
    '?logs=' + encodeURIComponent('req=1 status=200 dur=12ms\nreq=2 status=500 dur=88ms\ncache warm complete') +
    '&format=lines' +
    '&mask=wildcard' +
    '&extra_delimiters=' + encodeURIComponent('=') +
    '&min_count=2' +
    '&similarity=0.4' +
    '&depth=4';

  await page.goto('/tools/log-pattern-miner/' + qs);
  await expect(page.locator('#in-format')).toHaveValue('lines', { timeout: 15000 });
  await expect(page.locator('#in-mask')).toHaveValue('wildcard');
  await expect(page.locator('#in-extra_delimiters')).toHaveValue('=');
  await expect(page.locator('#in-min_count')).toHaveValue('2');
  await expect(page.locator('#tool-output')).toHaveText('req <*> status <*> dur <*>ms', { timeout: 20000 });
});

test('log-pattern-miner JSON includes examples and variable samples', async ({ page }) => {
  await page.goto('/tools/log-pattern-miner/');
  await fillMiner(page, { logs: sshLogs, format: 'json' });

  const raw = await page.locator('#tool-output').textContent({ timeout: 20000 });
  const report = JSON.parse(raw ?? '');
  expect(report.total_lines).toBe(4);
  expect(report.patterns_found).toBe(2);
  expect(report.patterns[0].count).toBe(3);
  expect(report.patterns[0].examples).toHaveLength(3);
  const ipSlot = report.patterns[0].variables.find((slot) => slot.placeholder === '<IP>');
  expect(ipSlot.values).toEqual(['10.0.0.1', '10.0.0.2', '10.0.0.3']);
});

test('log-pattern-miner reports invalid input clearly', async ({ page }) => {
  await page.goto('/tools/log-pattern-miner/');
  await fillMiner(page, { logs: 'one line only', similarity: '1.5' });

  await expect(page.locator('#tool-output')).toHaveClass(/error/, { timeout: 20000 });
  await expect(page.locator('#tool-output')).toContainText('similarity must be between 0 and 1');
});
