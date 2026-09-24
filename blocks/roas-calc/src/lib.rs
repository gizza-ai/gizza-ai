//! gizza-ai/roas-calc — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    ad_spend: f64,
    #[serde(default)]
    revenue: f64,
    #[serde(default)]
    conversions: f64,
    #[serde(default)]
    aov: f64,
    #[serde(default = "default_margin_basis")]
    margin_basis: String,
    #[serde(default = "default_gross_margin")]
    gross_margin: f64,
    #[serde(default)]
    cogs_per_order: f64,
    #[serde(default)]
    shipping_per_order: f64,
    #[serde(default = "default_payment_rate")]
    payment_rate: f64,
    #[serde(default = "default_payment_fixed")]
    payment_fixed: f64,
    #[serde(default)]
    refund_rate: f64,
    #[serde(default)]
    other_cost_per_order: f64,
    #[serde(default)]
    target_net_margin: f64,
    #[serde(default)]
    fixed_costs: f64,
    #[serde(default)]
    clicks: f64,
    #[serde(default)]
    cpc: f64,
    #[serde(default)]
    conversion_rate: f64,
    #[serde(default = "default_purchases")]
    purchases_per_customer: f64,
    #[serde(default)]
    purchase_interval_months: f64,
    #[serde(default)]
    revenue_goal: f64,
    #[serde(default = "default_true")]
    scenarios: bool,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default = "default_decimals")]
    decimals: f64,
    #[serde(default = "default_format")]
    format: String,
}

fn default_margin_basis() -> String {
    "percent".into()
}
fn default_gross_margin() -> f64 {
    50.0
}
fn default_payment_rate() -> f64 {
    2.9
}
fn default_payment_fixed() -> f64 {
    0.3
}
fn default_purchases() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}
fn default_currency() -> String {
    "$".into()
}
fn default_decimals() -> f64 {
    2.0
}
fn default_format() -> String {
    "markdown".into()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::number("ad_spend").required().min(0.01).describe(
            "Total amount spent on the campaign for one reporting period, in your chosen currency. Every other figure must cover the same period. Example: 10000.",
        ))
        .param(Param::number("revenue").required().min(0.0).default(0.0).describe(
            "Revenue attributed to that ad spend in the same period. Leave at 0 to derive it from aov multiplied by conversions. Example: 35000.",
        ))
        .param(Param::number("conversions").min(0.0).default(0.0).describe(
            "Orders or customers the ad spend produced. Needed for CAC, LTV and per-order figures. 0 means derive it from clicks and conversion_rate, or skip those sections. Example: 350.",
        ))
        .param(Param::number("aov").min(0.0).default(0.0).describe(
            "Average order value. 0 means derive it as revenue divided by conversions. Required when margin_basis is 'per_order' and conversions are unknown. Example: 100.",
        ))
        .param(Param::enumv("margin_basis", ["percent", "per_order"]).default("percent").describe(
            "How the contribution margin is established. 'percent' uses the single gross_margin percentage. 'per_order' builds it from the real per-order costs: cogs_per_order, shipping_per_order, payment_rate, payment_fixed, refund_rate and other_cost_per_order.",
        ))
        .param(Param::number("gross_margin").min(0.0).max(100.0).default(50.0).describe(
            "Gross margin on ad-driven revenue as a percentage, 0 to 100. Used only when margin_basis is 'percent'. Break-even ROAS is 1 divided by this margin, so 50 means a 2.00x break-even. Default 50.",
        ))
        .param(Param::number("cogs_per_order").min(0.0).default(0.0).describe(
            "Landed cost of goods for an average order, used when margin_basis is 'per_order'. Default 0.",
        ))
        .param(Param::number("shipping_per_order").min(0.0).default(0.0).describe(
            "Shipping, packaging and fulfilment cost for an average order, used when margin_basis is 'per_order'. Default 0.",
        ))
        .param(Param::number("payment_rate").min(0.0).max(100.0).default(2.9).describe(
            "Payment-processing percentage charged on each order, 0 to 100, used when margin_basis is 'per_order'. Default 2.9.",
        ))
        .param(Param::number("payment_fixed").min(0.0).default(0.3).describe(
            "Flat per-transaction payment fee charged on top of payment_rate, used when margin_basis is 'per_order'. Default 0.3.",
        ))
        .param(Param::number("refund_rate").min(0.0).max(100.0).default(0.0).describe(
            "Share of orders refunded or returned, 0 to 100. Deducted from the per-order margin as refund_rate percent of the order value. Used when margin_basis is 'per_order'. Default 0.",
        ))
        .param(Param::number("other_cost_per_order").min(0.0).default(0.0).describe(
            "Any other variable cost per order — inserts, affiliate fees, per-order app fees — used when margin_basis is 'per_order'. Default 0.",
        ))
        .param(Param::number("target_net_margin").min(0.0).max(100.0).default(0.0).describe(
            "Net margin you want the ad-driven revenue to leave, as a percentage. Sets the target ROAS to 1 divided by (contribution margin minus this target), and the most you can pay per order. Must stay below the contribution margin. 0 means no target. Default 0.",
        ))
        .param(Param::number("fixed_costs").min(0.0).default(0.0).describe(
            "Non-ad fixed costs for the same period — agency retainers, tooling, overhead — subtracted after the ad spend to give net profit. Default 0.",
        ))
        .param(Param::number("clicks").min(0.0).default(0.0).describe(
            "Clicks the ad spend bought. 0 means derive them from ad_spend divided by cpc, or skip the traffic section. Example: 5000.",
        ))
        .param(Param::number("cpc").min(0.0).default(0.0).describe(
            "Average cost per click. 0 means derive it from ad_spend divided by clicks. Supplying cpc with aov and conversion_rate forecasts a campaign that has not run yet. Example: 2.",
        ))
        .param(Param::number("conversion_rate").min(0.0).max(100.0).default(0.0).describe(
            "Share of clicks that become orders, 0 to 100. 0 means derive it from conversions divided by clicks. Example: 4 for 4 percent. Default 0.",
        ))
        .param(Param::number("purchases_per_customer").min(1.0).max(1000.0).default(1.0).describe(
            "Lifetime purchases an acquired customer makes, including the first — the LTV multiplier. Drives LTV, the LTV:CAC ratio and the LTV-adjusted ROAS. 1 means first order only. Default 1.",
        ))
        .param(Param::number("purchase_interval_months").min(0.0).max(120.0).default(0.0).describe(
            "Average months between repeat purchases. Combined with the purchases needed to repay CAC it gives a CAC payback period in months. 0 means no payback estimate. Default 0.",
        ))
        .param(Param::number("revenue_goal").min(0.0).default(0.0).describe(
            "Ad-driven revenue you want for the period. Adds the budget the goal needs at the current, break-even and target ROAS, plus the profit it would leave. 0 means no goal. Default 0.",
        ))
        .param(Param::boolean("scenarios").default(true).describe(
            "Include the what-if table showing profit per unit of spend, ad cost per order, profit per order and net profit at ROAS levels of 1x through 4x plus your break-even, current and target ROAS. Default true.",
        ))
        .param(Param::string("currency").default("$").describe(
            "Currency prefix or symbol used to label amounts, such as '$', '€', '£' or 'CHF '. Display only; no exchange rates are fetched. Default '$'.",
        ))
        .param(Param::integer("decimals").min(0.0).max(6.0).default(2).describe(
            "Decimal places for money and percentages, 0 to 6. ROAS multiples and ratios always show 2. Default 2.",
        ))
        .param(Param::enumv("format", ["markdown", "text", "csv", "json"]).default("markdown").describe(
            "Output format: markdown tables (default), aligned plain text, CSV rows, or JSON with every computed field.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/roas-calc",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute ROAS, ACOS, break-even and target ROAS, CAC, LTV and required ad budget",
    skill(
        description = "Turn one period of ad spend and attributed revenue into the full return-on-ad-spend picture. Reports gross ROAS, ROAS as a percentage, ACOS, net ROAS, break-even ROAS, a target ROAS for a chosen net margin, and an LTV-adjusted ROAS. The contribution margin can be a single gross-margin percentage or built from real per-order costs — cost of goods, shipping, payment rate plus fixed fee, a refund allowance and other variable costs. Adds profit after ad spend, net profit after fixed costs, net margin, profit per unit of spend, break-even ad spend and break-even revenue, CAC, the most you can pay per order at break-even and at your target margin, LTV, the LTV:CAC ratio, CAC payback, cost per click, break-even CPC, the budget a revenue goal needs at each ROAS, and a what-if table of profit at other ROAS levels. Forecasts an unlaunched campaign from aov, conversion_rate and cpc. Outputs markdown, text, CSV or JSON and runs locally. Educational planning arithmetic, not financial advice.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "roas-calc", |a: Args| {
            gizza_ai_roas_calc_core::run(
                a.ad_spend,
                a.revenue,
                a.conversions,
                a.aov,
                &a.margin_basis,
                a.gross_margin,
                a.cogs_per_order,
                a.shipping_per_order,
                a.payment_rate,
                a.payment_fixed,
                a.refund_rate,
                a.other_cost_per_order,
                a.target_net_margin,
                a.fixed_costs,
                a.clicks,
                a.cpc,
                a.conversion_rate,
                a.purchases_per_customer,
                a.purchase_interval_months,
                a.revenue_goal,
                a.scenarios,
                &a.currency,
                a.decimals,
                &a.format,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "ad_spend": { "type": "number", "minimum": 0.01, "description": "Total amount spent on the campaign for one reporting period, in your chosen currency. Every other figure must cover the same period. Example: 10000." },
                    "revenue": { "type": "number", "default": 0.0, "minimum": 0, "description": "Revenue attributed to that ad spend in the same period. Leave at 0 to derive it from aov multiplied by conversions. Example: 35000." },
                    "conversions": { "type": "number", "default": 0.0, "minimum": 0, "description": "Orders or customers the ad spend produced. Needed for CAC, LTV and per-order figures. 0 means derive it from clicks and conversion_rate, or skip those sections. Example: 350." },
                    "aov": { "type": "number", "default": 0.0, "minimum": 0, "description": "Average order value. 0 means derive it as revenue divided by conversions. Required when margin_basis is 'per_order' and conversions are unknown. Example: 100." },
                    "margin_basis": { "type": "string", "enum": ["percent", "per_order"], "default": "percent", "description": "How the contribution margin is established. 'percent' uses the single gross_margin percentage. 'per_order' builds it from the real per-order costs: cogs_per_order, shipping_per_order, payment_rate, payment_fixed, refund_rate and other_cost_per_order." },
                    "gross_margin": { "type": "number", "default": 50.0, "minimum": 0, "maximum": 100, "description": "Gross margin on ad-driven revenue as a percentage, 0 to 100. Used only when margin_basis is 'percent'. Break-even ROAS is 1 divided by this margin, so 50 means a 2.00x break-even. Default 50." },
                    "cogs_per_order": { "type": "number", "default": 0.0, "minimum": 0, "description": "Landed cost of goods for an average order, used when margin_basis is 'per_order'. Default 0." },
                    "shipping_per_order": { "type": "number", "default": 0.0, "minimum": 0, "description": "Shipping, packaging and fulfilment cost for an average order, used when margin_basis is 'per_order'. Default 0." },
                    "payment_rate": { "type": "number", "default": 2.9, "minimum": 0, "maximum": 100, "description": "Payment-processing percentage charged on each order, 0 to 100, used when margin_basis is 'per_order'. Default 2.9." },
                    "payment_fixed": { "type": "number", "default": 0.3, "minimum": 0, "description": "Flat per-transaction payment fee charged on top of payment_rate, used when margin_basis is 'per_order'. Default 0.3." },
                    "refund_rate": { "type": "number", "default": 0.0, "minimum": 0, "maximum": 100, "description": "Share of orders refunded or returned, 0 to 100. Deducted from the per-order margin as refund_rate percent of the order value. Used when margin_basis is 'per_order'. Default 0." },
                    "other_cost_per_order": { "type": "number", "default": 0.0, "minimum": 0, "description": "Any other variable cost per order — inserts, affiliate fees, per-order app fees — used when margin_basis is 'per_order'. Default 0." },
                    "target_net_margin": { "type": "number", "default": 0.0, "minimum": 0, "maximum": 100, "description": "Net margin you want the ad-driven revenue to leave, as a percentage. Sets the target ROAS to 1 divided by (contribution margin minus this target), and the most you can pay per order. Must stay below the contribution margin. 0 means no target. Default 0." },
                    "fixed_costs": { "type": "number", "default": 0.0, "minimum": 0, "description": "Non-ad fixed costs for the same period — agency retainers, tooling, overhead — subtracted after the ad spend to give net profit. Default 0." },
                    "clicks": { "type": "number", "default": 0.0, "minimum": 0, "description": "Clicks the ad spend bought. 0 means derive them from ad_spend divided by cpc, or skip the traffic section. Example: 5000." },
                    "cpc": { "type": "number", "default": 0.0, "minimum": 0, "description": "Average cost per click. 0 means derive it from ad_spend divided by clicks. Supplying cpc with aov and conversion_rate forecasts a campaign that has not run yet. Example: 2." },
                    "conversion_rate": { "type": "number", "default": 0.0, "minimum": 0, "maximum": 100, "description": "Share of clicks that become orders, 0 to 100. 0 means derive it from conversions divided by clicks. Example: 4 for 4 percent. Default 0." },
                    "purchases_per_customer": { "type": "number", "default": 1.0, "minimum": 1, "maximum": 1000, "description": "Lifetime purchases an acquired customer makes, including the first — the LTV multiplier. Drives LTV, the LTV:CAC ratio and the LTV-adjusted ROAS. 1 means first order only. Default 1." },
                    "purchase_interval_months": { "type": "number", "default": 0.0, "minimum": 0, "maximum": 120, "description": "Average months between repeat purchases. Combined with the purchases needed to repay CAC it gives a CAC payback period in months. 0 means no payback estimate. Default 0." },
                    "revenue_goal": { "type": "number", "default": 0.0, "minimum": 0, "description": "Ad-driven revenue you want for the period. Adds the budget the goal needs at the current, break-even and target ROAS, plus the profit it would leave. 0 means no goal. Default 0." },
                    "scenarios": { "type": "boolean", "default": true, "description": "Include the what-if table showing profit per unit of spend, ad cost per order, profit per order and net profit at ROAS levels of 1x through 4x plus your break-even, current and target ROAS. Default true." },
                    "currency": { "type": "string", "default": "$", "description": "Currency prefix or symbol used to label amounts, such as '$', '€', '£' or 'CHF '. Display only; no exchange rates are fetched. Default '$'." },
                    "decimals": { "type": "integer", "default": 2, "minimum": 0, "maximum": 6, "description": "Decimal places for money and percentages, 0 to 6. ROAS multiples and ratios always show 2. Default 2." },
                    "format": { "type": "string", "enum": ["markdown", "text", "csv", "json"], "default": "markdown", "description": "Output format: markdown tables (default), aligned plain text, CSV rows, or JSON with every computed field." }
                },
                "required": ["ad_spend", "revenue"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
