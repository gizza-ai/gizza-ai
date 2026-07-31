import { test, expect } from './fixtures';

const CISCO = [
  'interface GigabitEthernet0/1',
  ' description uplink',
  ' ip address 10.0.0.1 255.255.255.0',
  ' no shutdown',
  '!',
  'router ospf 1',
  ' network 10.0.0.0 0.0.0.255 area 0',
].join('\n');

const JUNOS = [
  'system {',
  '    host-name router1;',
  '    services {',
  '        ssh;',
  '    }',
  '}',
].join('\n');

async function setText(
  page: import('@playwright/test').Page,
  id: string,
  value: string,
) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('network-config-parser renders Cisco indentation config as an exact JSON tree', async ({ page }) => {
  await page.goto('/tools/network-config-parser/');
  await setText(page, '#in-config', CISCO);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('GigabitEthernet0/1', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`[
  {
    "children": [
      {
        "children": [],
        "line": "description uplink"
      },
      {
        "children": [],
        "line": "ip address 10.0.0.1 255.255.255.0"
      },
      {
        "children": [],
        "line": "no shutdown"
      }
    ],
    "line": "interface GigabitEthernet0/1"
  },
  {
    "children": [
      {
        "children": [],
        "line": "network 10.0.0.0 0.0.0.255 area 0"
      }
    ],
    "line": "router ospf 1"
  }
]`);
});

test('network-config-parser supports brace syntax and path output', async ({ page }) => {
  await page.goto('/tools/network-config-parser/');
  await setText(page, '#in-config', JUNOS);
  await page.selectOption('#in-output', 'paths');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('system / host-name router1', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`[
  "system / host-name router1",
  "system / services / ssh"
]`);
});

test('network-config-parser deep-link applies filter and report output', async ({ page }) => {
  const qs = new URLSearchParams({
    config: CISCO,
    syntax: 'indent',
    output: 'report',
    filter: 'ospf',
    comments: 'strip',
  });
  await page.goto(`/tools/network-config-parser/?${qs.toString()}`);

  await expect(page.locator('#in-config')).toHaveValue(CISCO);
  await expect(page.locator('#in-syntax')).toHaveValue('indent');
  await expect(page.locator('#in-output')).toHaveValue('report');
  await expect(page.locator('#in-filter')).toHaveValue('ospf');
  await expect(page.locator('#in-comments')).toHaveValue('strip');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('router ospf 1', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`{
  "sections": [
    "router ospf 1"
  ],
  "stats": {
    "comments": 1,
    "leaf_statements": 1,
    "max_depth": 2,
    "top_level_lines": 1,
    "top_level_sections": 1,
    "total_lines": 2
  },
  "syntax": "indent"
}`);
});
