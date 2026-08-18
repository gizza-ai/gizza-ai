import { test, expect } from './fixtures';

const tool = '/tools/email-network-analyzer/';
const sampleMbox = 'From alice@example.com Mon Jan  1 00:00:00 2024\nFrom: Alice <alice@example.com>\nTo: bob@example.com\nCc: carol@example.org\nDate: Tue, 2 Jan 2024 10:00:00 +0000\nSubject: kickoff\n\nhello\n\nFrom bob@example.com Mon Jan  1 00:00:00 2024\nFrom: Bob <bob@example.com>\nTo: alice@example.com\nDate: Wed, 3 Jan 2024 09:00:00 +0000\nSubject: re: kickoff\n\nack\n\nFrom alice@example.com Mon Jan  1 00:00:00 2024\nFrom: Alice <alice@example.com>\nTo: bob@example.com\nDate: Fri, 5 Jan 2024 11:30:00 +0000\nSubject: status\n\nupdate';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  input: string,
  me = '',
  nodes = 'address',
  recipients = 'to-cc',
  direction = 'directed',
  top = '10',
  minMessages = '1',
  exclude = '',
  selfLoops = 'false',
  since = '',
  until = '',
  format = 'report',
): Promise<string> {
  return await page.evaluate(
    async ({ input, me, nodes, recipients, direction, top, minMessages, exclude, selfLoops, since, until, format }) => {
      const mod = await import('/tools/email-network-analyzer/gizza_ai_email_network_analyzer_web.js');
      await mod.default('/tools/email-network-analyzer/gizza_ai_email_network_analyzer_web_bg.wasm');
      return mod.run(input, me, nodes, recipients, direction, top, minMessages, exclude, selfLoops, since, until, format);
    },
    { input, me, nodes, recipients, direction, top, minMessages, exclude, selfLoops, since, until, format },
  );
}

test('email-network-analyzer page renders a sender-recipient network report', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-input'), sampleMbox);
  await page.fill('#in-me', 'alice@example.com');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Email network', { timeout: 15_000 });
  await expect(out).toContainText('3 messages, 3 addresses, 3 links');
  await expect(out).toContainText('alice@example.com -> bob@example.com');
  await expect(out).toContainText('Your network — alice@example.com');
  await expect(out).toContainText('Reciprocity: 0.50 received per sent.');
});

test('email-network-analyzer deep link pre-fills and runs domain GraphML output', async ({ page }) => {
  const qs = new URLSearchParams({
    input: sampleMbox,
    nodes: 'domain',
    recipients: 'to-cc',
    direction: 'undirected',
    top: '5',
    min_messages: '1',
    self_loops: 'true',
    format: 'graphml',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue(sampleMbox, { timeout: 15_000 });
  await expect(page.locator('#in-nodes')).toHaveValue('domain');
  await expect(page.locator('#in-direction')).toHaveValue('undirected');
  await expect(page.locator('#in-self_loops')).toBeChecked();
  await expect(page.locator('#in-format')).toHaveValue('graphml');
  await expect(page.locator('#tool-output')).toContainText('<graphml');
  await expect(page.locator('#tool-output')).toContainText('edgedefault="undirected"');
});

test('email-network-analyzer wasm covers enums, formats, boundary, checkbox, and validation', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-input');

  const csv = await runWasm(page, sampleMbox, '', 'address', 'to', 'directed', '10', '1', '', 'false', '', '', 'csv');
  expect(csv).toContain('from,to,messages,first,last');
  expect(csv).toContain('alice@example.com,bob@example.com,2,2024-01-02,2024-01-05');
  expect(csv).not.toContain('carol@example.org');

  const undirected = JSON.parse(await runWasm(page, sampleMbox, '', 'address', 'to-cc', 'undirected', '100', '1', '', 'false', '', '', 'json'));
  expect(undirected.summary.direction).toBe('undirected');
  expect(undirected.edges[0].messages).toBe(3);

  const domainSelfLoops = JSON.parse(await runWasm(page, sampleMbox, '', 'domain', 'to-cc', 'directed', '10', '1', '', 'true', '', '', 'json'));
  expect(domainSelfLoops.edges.length).toBe(2);

  const dot = await runWasm(page, sampleMbox, '', 'address', 'to-cc-bcc', 'directed', '10', '2', '', 'false', '2024-01-01', '2024-12-31', 'dot');
  expect(dot).toContain('digraph email_network');
  expect(dot).toContain('alice@example.com');
  expect(dot).not.toContain('carol@example.org');

  await expect(runWasm(page, '', '')).rejects.toThrow(/input is empty/);
  await expect(runWasm(page, sampleMbox, '', 'people')).rejects.toThrow(/unknown nodes/);
  await expect(runWasm(page, sampleMbox, '', 'address', 'to-cc', 'directed', '101')).rejects.toThrow(/top must be between 1 and 100/);
  await expect(runWasm(page, sampleMbox, '', 'address', 'to-cc', 'directed', '10', '1', '', 'false', '2024-02-01', '2024-01-01')).rejects.toThrow(/is after until/);
});

test('email-network-analyzer ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(3);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Small mbox thread',
    'Domain-level view',
    'GraphML export for Gephi',
  ]);
});
