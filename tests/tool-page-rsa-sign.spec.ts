import { test, expect } from './fixtures';

// /tools/rsa-sign/ signs a message with an RSA private key in-browser (pure wasm).
// message + private_key are multiline <textarea>; scheme + hash are <select>.
// Test key is a throwaway RSA-2048 key generated for this test.
const KEY = `-----BEGIN PRIVATE KEY-----
MIIEugIBADANBgkqhkiG9w0BAQEFAASCBKQwggSgAgEAAoIBAQC8mwQTeVV07z4q
ozdP6tMpr6OSNXsRjylyGavkVZQvT66rbyWEHZMnn2BbwVOx4r51/ptDltZnRgAQ
ftGi8en5b2B5SBIxK0S1q8eCGW8ZYGwy3xxgl2M0gnSpdmZoGtydCgWUMHpWncpa
+ykDFWl7f1UFsIz3IJAVyu+pvvL7DIjxu1rDpNT1/AEeqvGHIhe/M+Dq1agSkC/+
LD56/lbGgxuWxOJ2NdbnrOSV8fXxj9y/mv+uCst0e4hIR484AMaSpdwJ57gARNBP
7CBLh3YSfnOxAIitHEqS40LCE7sMLnX8vmDw8qsUfaiu6l5b8+2TvG1sQ/+AF2+M
i2y03+7BAgMBAAECgf8xjhUZJ7j8q2C3w8Tb7Ib192SqLiGGCBsC4RxoJiqWVd5x
9vUUPXqqzjhZUlzut32v2fpqskXV7TwjXaqnwqrlaNtCLTu2KbQ5BGm8SCQZC+Rq
eFEFxnRJGY/yVaJ4d3ZZ6XVBh41l5DFA0Jc+SpLCJbapcvo9QCUKdvKs1sStPIMt
7Q8e8pHq1SEPrcCLQZey7/UmzAKPR8AAI523l5YLMCkZ/5eIlDPP/UuBpdpzQKAR
Pw2v/SsPycxcrc28mY0ll4wbhxa0w1H35gRmhn2x0xO5ktx5uZw9GEFGwCMgYXYM
/2EoqkOP7hgHgEFwSzGRil1nGrA39Kl5cp98AwECgYEA8yZnyu4Qp8sTL9RaxMVz
RRmEw5JU3CTFJcQ0SWm2RhH9HFfstZZVRdcIaZ7tgRxklsBnlLzUcB13KcM0kMGp
SMIJrGaa6d1nn7ap/aXknikWhARJhhfYPSq32tNEYjIvqL3j8FtAaAfwEYmfb5U0
NTH9WhrzfTEqq1zoskwizwECgYEAxpKsjq86kt00FdMXJGFkmFddyxrOWpcbScuC
4rc3YMxolIA0cTZNqwGBYtc7y6KMUZrZN6HJ4NWZTzpCcOLB/eOaBiGiDGu3pR+F
PvosAAgkqtFAwhFhHFo/Lm47XyvTa/Gcg4zfO1xnwv9E1EsRF37IjfARWkVmo0Tm
WkxQ38ECgYA/QAZP6425mEHtdzgFZ7eMig7XMQGSIp6GLHvNfQpFP/ivns+cjPax
rDYsA4OUymYAMRzAvD4mzANrgbPy0+3NV2xcxHQX0dha25FswfKukdGhldvqXdmK
T8pzyfFH+fYb1wmsRJCEf0wbw8kNpapnDBHzln8wWdHXsdt2RgfoAQKBgFDIvxbV
RvkUsOnoFNQiIzCu6mOEpkrIirt85eSiMQ9aWXmAptUgCHz3gdHaSmNP39IbMx+k
3GJxw4st4nXWaqGFhNXVvP1cnTu6FRVH5bqllXVA6B2LwHwuYuHayqCvTbXud4Q1
PWQC9duoyjGr0GpElAbakdStw1HM6AH4ZjaBAoGAMgEnktI0qNlmskD1SKUv0gGs
hl3Bustmr0QNQTpA0vR+K8TLuQR8AHPrHOXVtqoftiPliZzj+ncjENytU6+ADKIy
7uR/OePIgZyJL0UfP0DBXhFYeoZYzb0sbSyWhJWgjSV5+QyCjjM2f8ass9WCPxwD
aUUv2gS9W1LWjQlOFd0=
-----END PRIVATE KEY-----`;

test('rsa-sign page produces a base64 signature (pkcs1v15)', async ({ page }) => {
  await page.goto('/tools/rsa-sign/');
  await page.fill('#in-message', 'hello rsa');
  await page.fill('#in-private_key', KEY);
  await page.selectOption('#in-scheme', 'pkcs1v15');
  await page.selectOption('#in-hash', 'sha256');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const txt = (await out.textContent())!.trim();
  // RSA-2048 signature is 256 bytes -> 344 base64 chars.
  expect(txt).toMatch(/^[A-Za-z0-9+/]+=*$/);
  expect(txt.length).toBeGreaterThan(300);
});

test('rsa-sign page errors clearly on a bad key', async ({ page }) => {
  await page.goto('/tools/rsa-sign/');
  await page.fill('#in-message', 'hi');
  await page.fill('#in-private_key', 'not a key');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('private key', { timeout: 15000 });
});
