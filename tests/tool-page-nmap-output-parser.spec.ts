import { test, expect } from './fixtures';

const XML = '<?xml version="1.0"?>\n' +
  '<nmaprun><host><address addr="10.0.0.10" addrtype="ipv4"/>' +
  '<hostnames><hostname name="web.local" type="PTR"/></hostnames><ports>' +
  '<port protocol="tcp" portid="80"><state state="open"/><service name="http" product="nginx" version="1.18.0"/></port>' +
  '<port protocol="tcp" portid="443"><state state="closed"/><service name="https"/></port>' +
  '</ports></host></nmaprun>';

const GREP = 'Host: 10.0.0.9 (router.local)\tPorts: 22/open/tcp//ssh//OpenSSH 8.2p1/, 80/open/tcp//http//nginx 1.18.0/';

test('nmap-output-parser parses XML to markdown and filters closed ports', async ({ page }) => {
  await page.goto('/tools/nmap-output-parser/');
  await page.fill('#in-input', XML);
  await page.selectOption('#in-format', 'xml');
  await page.selectOption('#in-output', 'markdown');
  await page.selectOption('#in-sort_by', 'port');
  await expect(page.locator('#tool-output')).toContainText('| 10.0.0.10 | web.local | 80', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('nginx 1.18.0');
  await expect(page.locator('#tool-output')).not.toContainText('443');
});

test('nmap-output-parser can include closed ports and render CSV', async ({ page }) => {
  await page.goto('/tools/nmap-output-parser/');
  await page.fill('#in-input', XML);
  await page.selectOption('#in-format', 'xml');
  await page.selectOption('#in-output', 'csv');
  await page.uncheck('#in-open_only');
  await expect(page.locator('#tool-output')).toContainText('Host,Hostname,Port,Protocol,State,Service,Version', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('10.0.0.10,web.local,443,tcp,closed,https');
});

test('nmap-output-parser parses greppable output to JSON', async ({ page }) => {
  await page.goto('/tools/nmap-output-parser/');
  await page.fill('#in-input', GREP);
  await page.selectOption('#in-format', 'greppable');
  await page.selectOption('#in-output', 'json');
  await page.selectOption('#in-sort_by', 'service');
  await expect(page.locator('#tool-output')).toContainText('"host": "10.0.0.9"', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('"service": "http"');
  await expect(page.locator('#tool-output')).toContainText('"service": "ssh"');
});

test('nmap-output-parser query-param deep-link', async ({ page }) => {
  await page.goto(
    '/tools/nmap-output-parser/?input=' +
      encodeURIComponent(GREP) +
      '&format=greppable&output=csv&sort_by=port&open_only=true',
  );
  await expect(page.locator('#in-input')).toHaveValue(GREP, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('10.0.0.9,router.local,22,tcp,open,ssh,OpenSSH 8.2p1', { timeout: 15000 });
});
