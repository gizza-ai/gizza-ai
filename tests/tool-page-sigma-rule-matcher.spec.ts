import { test, expect } from './fixtures';

const tool = '/tools/sigma-rule-matcher/';

const psRule = `title: Encoded PowerShell
id: demo-001
status: test
level: high
detection:
  sel:
    EventID: 4104
    ScriptBlockText|contains:
      - '-enc '
      - '-EncodedCommand'
  condition: sel`;

const psEvents = '[{"EventID":4104,"ScriptBlockText":"powershell.exe -enc SQBFAFgA"},{"EventID":4104,"ScriptBlockText":"Get-ChildItem"}]';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  rules: string,
  events: string,
  minLevel = 'any',
  status = 'any',
  output = 'report',
  maxMatches = '500',
  showEvent = 'false',
): Promise<string> {
  return await page.evaluate(
    async ({ rules, events, minLevel, status, output, maxMatches, showEvent }) => {
      const mod = await import('/tools/sigma-rule-matcher/gizza_ai_sigma_rule_matcher_web.js');
      await mod.default('/tools/sigma-rule-matcher/gizza_ai_sigma_rule_matcher_web_bg.wasm');
      return mod.run(rules, events, minLevel, status, output, maxMatches, showEvent);
    },
    { rules, events, minLevel, status, output, maxMatches, showEvent },
  );
}

test('sigma-rule-matcher page reports a Sigma hit with exact text', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-rules'), psRule);
  await setTextarea(page.locator('#in-events'), psEvents);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Sigma detection report', { timeout: 15_000 });
  await expect(out).toContainText('Rules loaded:   1');
  await expect(out).toContainText('Events scanned: 2');
  await expect(out).toContainText('Detections:     1 (1 of 2 events matched)');
  await expect(out).toContainText('1. [high] Encoded PowerShell — event 1');
});

test('sigma-rule-matcher deep link pre-fills and runs JSON output', async ({ page }) => {
  const qs = new URLSearchParams({
    rules: psRule,
    events: psEvents,
    min_level: 'high',
    status: 'test',
    output: 'json',
    max_matches: '5',
    show_event: 'true',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-rules')).toHaveValue(psRule, { timeout: 15_000 });
  await expect(page.locator('#in-events')).toHaveValue(psEvents);
  await expect(page.locator('#in-min_level')).toHaveValue('high');
  await expect(page.locator('#in-status')).toHaveValue('test');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-max_matches')).toHaveValue('5');
  await expect(page.locator('#in-show_event')).toBeChecked();

  const text = await page.locator('#tool-output').textContent({ timeout: 15_000 });
  const json = JSON.parse(text || '{}');
  expect(json.summary.detections).toBe(1);
  expect(json.detections[0].title).toBe('Encoded PowerShell');
  expect(json.detections[0].event.EventID).toBe(4104);
});

test('sigma-rule-matcher wasm covers modifiers, filters, caps and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-rules');

  const cidrRule = `title: CIDR
level: medium
status: stable
detection:
  sel:
    DestinationIp|cidr: '10.0.0.0/8'
  condition: sel`;
  expect(await runWasm(page, cidrRule, '[{"DestinationIp":"10.2.3.4"}]', 'medium', 'stable')).toContain('Detections:     1');
  expect(await runWasm(page, cidrRule, '[{"DestinationIp":"10.2.3.4"}]', 'critical', 'stable')).toContain('Rules loaded:   0');

  const twoHits = '[{"EventID":4104,"ScriptBlockText":"powershell -enc A"},{"EventID":4104,"ScriptBlockText":"powershell -EncodedCommand B"}]';
  const capped = await runWasm(page, psRule, twoHits, 'any', 'any', 'report', '1');
  expect(capped).toContain('Detections:     2 (2 of 2 events matched)');
  expect(capped).toContain('… 1 more detection(s) not shown (max_matches = 1)');

  await expect(runWasm(page, psRule, 'not json')).rejects.toThrow(/line 1 is not valid JSON/);
  await expect(runWasm(page, psRule, psEvents, 'bogus')).rejects.toThrow(/unknown min_level/);
});

test('sigma-rule-matcher ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(3);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Encoded PowerShell hit',
    'Failed logon JSON output',
    'Suspicious process table',
  ]);
});
