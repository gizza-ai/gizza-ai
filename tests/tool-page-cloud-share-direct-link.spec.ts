import { test, expect } from './fixtures';

const dropboxInput = 'https://www.dropbox.com/scl/fi/abc123/report.pdf?rlkey=xyz&st=aa&dl=0';
const dropboxOutput = 'https://www.dropbox.com/scl/fi/abc123/report.pdf?rlkey=xyz&st=aa&dl=1';

test('cloud-share-direct-link page rewrites Dropbox share links exactly', async ({ page }) => {
  await page.goto('/tools/cloud-share-direct-link/');
  await page.fill('#in-url', dropboxInput);

  await expect(page.locator('#tool-output')).toHaveText(dropboxOutput, { timeout: 15_000 });
});

test('cloud-share-direct-link honours deep-link params for Nextcloud curl output', async ({ page }) => {
  const params = new URLSearchParams({
    url: 'https://cloud.example.com/s/yxcFKRWBJqYYzp4',
    provider: 'auto',
    mode: 'download',
    output: 'curl',
    onedrive_style: 'api',
    docs_export: 'pdf',
    file: 'reports/Q3 report.pdf',
    per_line: 'false',
  });

  await page.goto(`/tools/cloud-share-direct-link/?${params.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('curl', { timeout: 15_000 });
  await expect(page.locator('#in-file')).toHaveValue('reports/Q3 report.pdf');
  await expect(page.locator('#in-per_line')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    'curl -L -o "Q3 report.pdf" "https://cloud.example.com/s/yxcFKRWBJqYYzp4/download?path=%2Freports&files=Q3%20report.pdf"',
    { timeout: 15_000 },
  );
});

test('cloud-share-direct-link covers enum choices and batch checkbox', async ({ page }) => {
  await page.goto('/tools/cloud-share-direct-link/');

  await page.fill('#in-url', `https://drive.google.com/file/d/1A2b3C4d5E6f7G8h9I0jKlMnOpQrStUvW/view?usp=sharing\n\nhttps://cloud.example.com/s/TOKEN123`);
  await expect(page.locator('#tool-output')).toHaveText(
    `https://drive.usercontent.google.com/download?id=1A2b3C4d5E6f7G8h9I0jKlMnOpQrStUvW&export=download&confirm=t\n\nhttps://cloud.example.com/s/TOKEN123/download`,
    { timeout: 15_000 },
  );

  await page.uncheck('#in-per_line');
  await page.fill('#in-url', dropboxInput);
  await page.selectOption('#in-mode', 'inline');
  await page.selectOption('#in-output', 'markdown');
  await expect(page.locator('#tool-output')).toHaveText(
    '[report.pdf](https://www.dropbox.com/scl/fi/abc123/report.pdf?rlkey=xyz&st=aa&raw=1)',
    { timeout: 15_000 },
  );

  await page.fill('#in-url', 'https://1drv.ms/t/s!AhJrpDQRn5d?e=YFQlMA');
  await page.selectOption('#in-mode', 'download');
  await page.selectOption('#in-output', 'url');
  await page.selectOption('#in-onedrive_style', 'download_param');
  await expect(page.locator('#tool-output')).toHaveText(
    'https://1drv.ms/t/s!AhJrpDQRn5d?e=YFQlMA&download=1',
    { timeout: 15_000 },
  );

  await page.fill('#in-url', 'https://docs.google.com/spreadsheets/d/1A2b3C4d5E6f7G8h9I0jKlMnOpQrStUvW/edit');
  await page.selectOption('#in-docs_export', 'office');
  await expect(page.locator('#tool-output')).toHaveText(
    'https://docs.google.com/spreadsheets/d/1A2b3C4d5E6f7G8h9I0jKlMnOpQrStUvW/export?format=xlsx',
    { timeout: 15_000 },
  );
});
