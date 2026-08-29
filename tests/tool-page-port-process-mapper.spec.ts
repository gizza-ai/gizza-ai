import { test, expect } from './fixtures';

const SS = [
  'Netid State  Recv-Q Send-Q Local Address:Port Peer Address:Port Process',
  'tcp   LISTEN 0      128          0.0.0.0:22        0.0.0.0:*    users:(("sshd",pid=575,fd=3))',
  'tcp   LISTEN 0      511        127.0.0.1:8080      0.0.0.0:*    users:(("nginx",pid=1234,fd=6))',
  'tcp   LISTEN 0      511          0.0.0.0:8080      0.0.0.0:*    users:(("node",pid=4321,fd=20))',
  'udp   UNCONN 0      0            0.0.0.0:68        0.0.0.0:*    users:(("dhclient",pid=812,fd=6))',
].join('\n');

const WIN = [
  'Active Connections',
  '',
  '  Proto  Local Address          Foreign Address        State           PID',
  '  TCP    0.0.0.0:3000           0.0.0.0:0              LISTENING       1234',
  '  TCP    127.0.0.1:3000         0.0.0.0:0              LISTENING       5678',
  '  UDP    0.0.0.0:5353           *:*                                    2500',
].join('\n');

async function setInput(page: any, value: string) {
  await page.$eval(
    '#in-input',
    (el: HTMLTextAreaElement, v: string) => {
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    value,
  );
}

test('port-process-mapper page maps ss output to a table and flags the contended port', async ({ page }) => {
  await page.goto('/tools/port-process-mapper/');
  await setInput(page, SS);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('| tcp | 0.0.0.0 | 22 | ssh | LISTEN | 575 | sshd | - | no |', { timeout: 15000 });
  await expect(out).toContainText('| udp | 0.0.0.0 | 68 | dhcp-client | UNCONN | 812 | dhclient | - | no |');
  await expect(out).toContainText('| tcp | 127.0.0.1 | 8080 | http-alt (dev server/proxy) | LISTEN | 1234 | nginx | - | yes |');
  await expect(out).toContainText('**Summary:** 4 rows, 4 listening, 3 unique ports, 1 conflict, parsed as ss');
  await expect(out).toContainText(
    '- tcp port 8080 is bound by 2 processes: nginx (PID 1234) on 127.0.0.1, node (PID 4321) on 0.0.0.0',
  );
});

test('port-process-mapper page emits kill commands when the checkbox is ticked', async ({ page }) => {
  await page.goto('/tools/port-process-mapper/');
  await setInput(page, SS);
  await page.check('#in-kill_commands');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('## Free a port', { timeout: 15000 });
  await expect(out).toContainText(
    'tcp 8080 (nginx PID 1234, node PID 4321) — Linux/macOS: kill -9 1234 4321 · Windows: taskkill /PID 1234 /PID 4321 /F',
  );
});

test('port-process-mapper query params prefill every control and compute a windows CSV deep link', async ({ page }) => {
  await page.goto(
    '/tools/port-process-mapper/?input=' +
      encodeURIComponent(WIN) +
      '&input_format=netstat-windows&output_format=csv&sort_by=pid&listening_only=true' +
      '&protocol=tcp&ports=3000&process=&conflicts_only=true&annotate_services=false&kill_commands=false',
  );

  await expect(page.locator('#in-input')).toHaveValue(WIN, { timeout: 15000 });
  await expect(page.locator('#in-input_format')).toHaveValue('netstat-windows');
  await expect(page.locator('#in-output_format')).toHaveValue('csv');
  await expect(page.locator('#in-sort_by')).toHaveValue('pid');
  await expect(page.locator('#in-protocol')).toHaveValue('tcp');
  await expect(page.locator('#in-ports')).toHaveValue('3000');
  await expect(page.locator('#in-listening_only')).toBeChecked();
  await expect(page.locator('#in-conflicts_only')).toBeChecked();
  await expect(page.locator('#in-annotate_services')).not.toBeChecked();

  // annotate_services=false drops the service column; conflicts_only + ports=3000
  // + protocol=tcp leaves exactly the two PIDs contending for port 3000.
  const out = page.locator('#tool-output');
  await expect(out).toContainText('proto,address,port,state,pid,process,user,peer,conflict', { timeout: 15000 });
  await expect(out).toContainText('tcp,0.0.0.0,3000,LISTEN,1234,-,-,,yes');
  await expect(out).toContainText('tcp,127.0.0.1,3000,LISTEN,5678,-,-,,yes');
  await expect(out).not.toContainText('5353');
});
