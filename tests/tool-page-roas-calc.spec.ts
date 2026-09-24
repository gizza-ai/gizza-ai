import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  adSpend = '10000',
  revenue = '35000',
  conversions = '0',
  aov = '0',
  marginBasis = 'percent',
  grossMargin = '50',
  cogsPerOrder = '0',
  shippingPerOrder = '0',
  paymentRate = '2.9',
  paymentFixed = '0.3',
  refundRate = '0',
  otherCostPerOrder = '0',
  targetNetMargin = '0',
  fixedCosts = '0',
  clicks = '0',
  cpc = '0',
  conversionRate = '0',
  purchasesPerCustomer = '1',
  purchaseIntervalMonths = '0',
  revenueGoal = '0',
  scenarios = 'false',
  currency = '$',
  decimals = '2',
  format = 'json',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/roas-calc/gizza_ai_roas_calc_web.js');
    await mod.default('/tools/roas-calc/gizza_ai_roas_calc_web_bg.wasm');
    return mod.run(
      args.adSpend,
      args.revenue,
      args.conversions,
      args.aov,
      args.marginBasis,
      args.grossMargin,
      args.cogsPerOrder,
      args.shippingPerOrder,
      args.paymentRate,
      args.paymentFixed,
      args.refundRate,
      args.otherCostPerOrder,
      args.targetNetMargin,
      args.fixedCosts,
      args.clicks,
      args.cpc,
      args.conversionRate,
      args.purchasesPerCustomer,
      args.purchaseIntervalMonths,
      args.revenueGoal,
      args.scenarios,
      args.currency,
      args.decimals,
      args.format,
    );
  }, { adSpend, revenue, conversions, aov, marginBasis, grossMargin, cogsPerOrder, shippingPerOrder, paymentRate, paymentFixed, refundRate, otherCostPerOrder, targetNetMargin, fixedCosts, clicks, cpc, conversionRate, purchasesPerCustomer, purchaseIntervalMonths, revenueGoal, scenarios, currency, decimals, format });
}

const BASE_JSON = `{
  "ad_spend": 10000,
  "revenue": 35000,
  "aov": null,
  "margin_basis": "percent",
  "contribution_margin_percent": 50,
  "contribution_margin_per_order": null,
  "roas": 3.5,
  "roas_percent": 350,
  "acos_percent": 28.57,
  "net_roas": 1.75,
  "break_even_roas": 2,
  "target_roas": null,
  "ltv_roas": null,
  "gross_profit": 17500,
  "profit_after_ads": 7500,
  "fixed_costs": 0,
  "net_profit": 7500,
  "net_margin_percent": 21.43,
  "profit_per_spend": 0.75,
  "cost_per_revenue": 0.29,
  "break_even_spend": 17500,
  "break_even_revenue": 20000,
  "headroom_percent": 75,
  "verdict": "Above break-even: every unit of ad spend returns 0.75 of contribution margin after paying for itself, with 75.0% of headroom before the campaign stops paying.",
  "customer": null,
  "clicks": null,
  "goal": null,
  "scenarios": []
}`;

test('roas-calc wasm computes baseline JSON exactly', async ({ page }) => {
  await page.goto('/tools/roas-calc/');
  await page.waitForSelector('#in-ad_spend');

  await expect(runWasm(page)).resolves.toBe(BASE_JSON);
});

test('roas-calc wasm covers per-order margin, CAC and LTV fields', async ({ page }) => {
  await page.goto('/tools/roas-calc/');
  await page.waitForSelector('#in-ad_spend');

  const out = await runWasm(
    page,
    '4000',
    '0',
    '250',
    '80',
    'per_order',
    '0',
    '28',
    '7',
    '2.9',
    '0.30',
    '3',
    '0',
    '15',
    '0',
    '0',
    '0',
    '0',
    '3',
    '2',
    '0',
    'false',
    '$',
    '2',
    'json',
  );
  expect(out).toContain('"margin_basis": "per_order"');
  expect(out).toContain('"contribution_margin_per_order": 39.98');
  expect(out).toContain('"cac": 16');
  expect(out).toContain('"ltv_cac": 7.5');
  expect(out).toContain('"payback_months": 0.8');
});

test('roas-calc page renders exact output and honors controls', async ({ page }) => {
  await page.goto('/tools/roas-calc/');
  await page.locator('#in-ad_spend').fill('10000');
  await page.locator('#in-revenue').fill('35000');
  await page.locator('#in-gross_margin').fill('50');
  await page.locator('#in-scenarios').uncheck();
  await page.locator('#in-format').selectOption('json');
  await expect(page.locator('#tool-output')).toHaveText(BASE_JSON, { timeout: 15_000 });
});

test('roas-calc deep-link prefills controls and runs exact output', async ({ page }) => {
  const params = new URLSearchParams({
    ad_spend: '10000',
    revenue: '35000',
    gross_margin: '50',
    scenarios: 'false',
    format: 'json',
  });

  await page.goto(`/tools/roas-calc/?${params.toString()}`);
  await expect(page.locator('#in-ad_spend')).toHaveValue('10000', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toHaveText(BASE_JSON, { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool roas-calc');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
