import { test, expect } from './fixtures';

const tool = '/tools/hcl-to-json/';
const resource = 'resource "aws_instance" "web" {\n  ami = "ami-0123456789"\n  instance_type = "t3.micro"\n  monitoring = true\n  tags = { Name = "web", Env = "prod" }\n}';

test('hcl-to-json page converts Terraform resource blocks', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-hcl', resource);
  await expect(page.locator('#tool-output')).toContainText('"resource"', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('"aws_instance"');
  await expect(page.locator('#tool-output')).toContainText('"web"');
  await expect(page.locator('#tool-output')).toContainText('"ami": "ami-0123456789"');
  await expect(page.locator('#tool-output')).toContainText('"monitoring": true');
});

test('hcl-to-json page supports array block shape', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-hcl', 'provisioner "local-exec" {\n  command = "a"\n}\nprovisioner "local-exec" {\n  command = "b"\n}');
  await page.selectOption('#in-blocks', 'arrays');
  await expect(page.locator('#tool-output')).toContainText('"local-exec"', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('"command": "a"');
  await expect(page.locator('#tool-output')).toContainText('"command": "b"');
});

test('hcl-to-json page simplifies constants and preserves variables', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-hcl', 'locals {\n  replicas = 1 + 2\n  keep = var.region\n}');
  await page.selectOption('#in-expressions', 'simplify');
  await expect(page.locator('#tool-output')).toContainText('"replicas": 3', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('"keep": "${var.region}"');
});

test('hcl-to-json query-param deep-link prefills and computes compact sorted output', async ({ page }) => {
  await page.goto(
    tool +
      '?hcl=' +
      encodeURIComponent('zeta = 1\nalpha = 2') +
      '&sort_keys=true&pretty=false&indent=4',
  );
  await expect(page.locator('#in-hcl')).toHaveValue('zeta = 1\nalpha = 2', { timeout: 15000 });
  await expect(page.locator('#in-sort_keys')).toBeChecked();
  await expect(page.locator('#in-pretty')).not.toBeChecked();
  await expect(page.locator('#in-indent')).toHaveValue('4');
  await expect(page.locator('#tool-output')).toContainText('{"alpha":2,"zeta":1}');
});
