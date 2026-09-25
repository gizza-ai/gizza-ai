//! roas-calc core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Turns a campaign's ad spend and attributed revenue into the full return-on-ad-spend
//! picture: gross/net/break-even/target/LTV-adjusted ROAS, ACOS, contribution margin per
//! order, CAC and the maximum CAC the margin can pay for, LTV and the LTV:CAC ratio, the
//! break-even ad spend and revenue, the budget a revenue goal needs, and a ladder of
//! what-if ROAS scenarios.
//!
//! Two margin models are supported. `percent` takes a single gross-margin percentage.
//! `per_order` builds the contribution margin from the real per-order costs — cost of
//! goods, shipping, payment processing (rate + fixed fee), a refund allowance and any
//! other variable cost — which is what an ecommerce break-even ROAS actually depends on.

/// Largest money value accepted for any single input.
const MAX_MONEY: f64 = 1e12;
/// Largest count accepted for orders/clicks.
const MAX_COUNT: f64 = 1e12;
/// Tolerance for "is this the same ROAS point" and for break-even comparisons.
const EPS: f64 = 1e-9;

/// Where the per-order contribution margin went, when `margin_basis = per_order`.
#[derive(Debug, Clone, PartialEq)]
pub struct CostBreakdown {
    pub cogs: f64,
    pub shipping: f64,
    pub processing: f64,
    pub refund_allowance: f64,
    pub other: f64,
    pub total: f64,
}

/// Traffic economics, present when clicks (or a CPC to derive them from) are known.
#[derive(Debug, Clone, PartialEq)]
pub struct Clicks {
    pub clicks: f64,
    pub cpc: f64,
    pub conversion_rate: f64,
    pub clicks_per_order: Option<f64>,
    pub revenue_per_click: f64,
    pub margin_per_click: f64,
    pub break_even_cpc: Option<f64>,
}

/// Per-customer economics, present when the order/customer count is known.
#[derive(Debug, Clone, PartialEq)]
pub struct Customer {
    pub conversions: f64,
    pub cac: f64,
    pub max_cac: Option<f64>,
    pub max_cac_at_target: Option<f64>,
    pub cac_headroom: Option<f64>,
    pub purchases_per_customer: f64,
    pub ltv_revenue: Option<f64>,
    pub ltv_margin: Option<f64>,
    pub ltv_cac: Option<f64>,
    pub purchases_to_repay: Option<f64>,
    pub payback_months: Option<f64>,
}

/// What it takes to hit an explicit revenue goal.
#[derive(Debug, Clone, PartialEq)]
pub struct Goal {
    pub revenue_goal: f64,
    pub extra_revenue: f64,
    pub orders_needed: Option<f64>,
    pub budget_at_current: f64,
    pub extra_spend: f64,
    pub budget_at_break_even: f64,
    pub budget_at_target: Option<f64>,
    pub profit_at_goal: f64,
}

/// One rung of the what-if ROAS ladder.
#[derive(Debug, Clone, PartialEq)]
pub struct Scenario {
    pub roas: f64,
    pub note: String,
    pub profit_per_spend: f64,
    pub ad_cost_per_order: Option<f64>,
    pub profit_per_order: Option<f64>,
    pub net_profit: f64,
}

/// Every figure the calculator produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    // resolved inputs
    pub ad_spend: f64,
    pub revenue: f64,
    pub aov: Option<f64>,
    pub margin_basis: String,
    // margin
    pub cm_percent: f64,
    pub cm_per_order: Option<f64>,
    pub costs: Option<CostBreakdown>,
    // the ROAS family
    pub gross_roas: f64,
    pub roas_percent: f64,
    pub acos: f64,
    pub cost_per_revenue: f64,
    pub net_roas: f64,
    pub break_even_roas: f64,
    pub target_net_margin: f64,
    pub target_roas: Option<f64>,
    pub ltv_roas: Option<f64>,
    // profit
    pub gross_profit: f64,
    pub profit_after_ads: f64,
    pub fixed_costs: f64,
    pub net_profit: f64,
    pub net_margin: f64,
    pub profit_per_spend: f64,
    pub break_even_spend: f64,
    pub break_even_revenue: f64,
    pub headroom_percent: f64,
    pub verdict: String,
    // optional blocks
    pub customer: Option<Customer>,
    pub clicks: Option<Clicks>,
    pub goal: Option<Goal>,
    pub scenarios: Vec<Scenario>,
}

/// Validate a money-shaped input.
fn money(v: f64, name: &str) -> Result<f64, String> {
    if !v.is_finite() {
        return Err(format!("{name} must be a finite number"));
    }
    if v < 0.0 {
        return Err(format!("{name} must be zero or positive"));
    }
    if v > MAX_MONEY {
        return Err(format!("{name} must be at most {MAX_MONEY:.0}"));
    }
    Ok(v)
}

/// Validate a percentage-shaped input.
fn percent(v: f64, name: &str, max: f64) -> Result<f64, String> {
    if !v.is_finite() {
        return Err(format!("{name} must be a finite number"));
    }
    if v < 0.0 || v > max {
        return Err(format!("{name} must be between 0 and {max:.0} percent"));
    }
    Ok(v)
}

/// Validate a count-shaped input (orders, clicks).
fn count(v: f64, name: &str) -> Result<f64, String> {
    if !v.is_finite() {
        return Err(format!("{name} must be a finite number"));
    }
    if v < 0.0 {
        return Err(format!("{name} must be zero or positive"));
    }
    if v > MAX_COUNT {
        return Err(format!("{name} must be at most {MAX_COUNT:.0}"));
    }
    Ok(v)
}

/// Add a ROAS point to the scenario ladder, merging duplicate rungs and their notes.
fn add_point(pts: &mut Vec<(f64, Vec<String>)>, v: f64, note: Option<&str>) {
    if !v.is_finite() || v <= 0.0 || v > 1e6 {
        return;
    }
    let key = (v * 10_000.0).round() / 10_000.0;
    for p in pts.iter_mut() {
        if (p.0 - key).abs() < EPS {
            if let Some(n) = note {
                if !p.1.iter().any(|x| x == n) {
                    p.1.push(n.to_string());
                }
            }
            return;
        }
    }
    pts.push((
        key,
        note.map(|n| vec![n.to_string()]).unwrap_or_default(),
    ));
}

/// Compute the whole report. All money arguments are in one currency and cover the
/// same reporting period (a day, a week, a month — the calculator never assumes one).
///
/// - `ad_spend`  — total spend on the campaign. Required, greater than 0.
/// - `revenue`   — revenue attributed to that spend. 0 means derive it from `aov` × orders.
/// - `conversions` — orders or customers the spend produced. 0 means derive or skip CAC.
/// - `aov`       — average order value. 0 means derive it from revenue ÷ orders.
/// - `margin_basis` — `percent` (use `gross_margin`) or `per_order` (build it from costs).
/// - `target_net_margin` — the net margin the campaign should leave, driving the target ROAS.
/// - `fixed_costs` — non-ad overhead for the period, subtracted after the ad spend.
/// - `clicks`, `cpc`, `conversion_rate` — any two of the three fill in the traffic block.
/// - `purchases_per_customer` — lifetime purchases per acquired customer (the LTV multiplier).
/// - `purchase_interval_months` — months between repeat purchases, for CAC payback. 0 = off.
/// - `revenue_goal` — revenue target for the period, driving the required-budget block. 0 = off.
#[allow(clippy::too_many_arguments)]
pub fn compute(
    ad_spend: f64,
    revenue: f64,
    conversions: f64,
    aov: f64,
    margin_basis: &str,
    gross_margin: f64,
    cogs_per_order: f64,
    shipping_per_order: f64,
    payment_rate: f64,
    payment_fixed: f64,
    refund_rate: f64,
    other_cost_per_order: f64,
    target_net_margin: f64,
    fixed_costs: f64,
    clicks: f64,
    cpc: f64,
    conversion_rate: f64,
    purchases_per_customer: f64,
    purchase_interval_months: f64,
    revenue_goal: f64,
    scenarios: bool,
) -> Result<Report, String> {
    // ---- validate ----------------------------------------------------------
    let ad_spend = money(ad_spend, "ad_spend")?;
    if ad_spend <= 0.0 {
        return Err("ad_spend must be greater than 0".into());
    }
    let revenue_in = money(revenue, "revenue")?;
    let conversions_in = count(conversions, "conversions")?;
    let aov_in = money(aov, "aov")?;
    let gross_margin = percent(gross_margin, "gross_margin", 100.0)?;
    let cogs_per_order = money(cogs_per_order, "cogs_per_order")?;
    let shipping_per_order = money(shipping_per_order, "shipping_per_order")?;
    let payment_rate = percent(payment_rate, "payment_rate", 100.0)?;
    let payment_fixed = money(payment_fixed, "payment_fixed")?;
    let refund_rate = percent(refund_rate, "refund_rate", 100.0)?;
    let other_cost_per_order = money(other_cost_per_order, "other_cost_per_order")?;
    let target_net_margin = percent(target_net_margin, "target_net_margin", 100.0)?;
    let fixed_costs = money(fixed_costs, "fixed_costs")?;
    let clicks_in = count(clicks, "clicks")?;
    let cpc_in = money(cpc, "cpc")?;
    let conversion_rate_in = percent(conversion_rate, "conversion_rate", 100.0)?;
    let revenue_goal = money(revenue_goal, "revenue_goal")?;

    if !purchases_per_customer.is_finite() || !(1.0..=1000.0).contains(&purchases_per_customer) {
        return Err("purchases_per_customer must be between 1 and 1000".into());
    }
    if !purchase_interval_months.is_finite() || !(0.0..=120.0).contains(&purchase_interval_months) {
        return Err("purchase_interval_months must be between 0 and 120".into());
    }

    let basis = margin_basis.trim().to_ascii_lowercase();
    let per_order_basis = match basis.as_str() {
        "percent" => false,
        "per_order" => true,
        other => {
            return Err(format!(
                "margin_basis must be 'percent' or 'per_order', got '{other}'"
            ))
        }
    };

    // ---- resolve the input chain -------------------------------------------
    // clicks → conversions → revenue → aov, each falling back to what it can be
    // derived from, so partial real-world data still produces a full report.
    let clicks_resolved = if clicks_in > 0.0 {
        clicks_in
    } else if cpc_in > 0.0 {
        ad_spend / cpc_in
    } else {
        0.0
    };

    let conversions_resolved = if conversions_in > 0.0 {
        conversions_in
    } else if clicks_resolved > 0.0 && conversion_rate_in > 0.0 {
        clicks_resolved * conversion_rate_in / 100.0
    } else {
        0.0
    };

    let revenue = if revenue_in > 0.0 {
        revenue_in
    } else if aov_in > 0.0 && conversions_resolved > 0.0 {
        aov_in * conversions_resolved
    } else {
        return Err(
            "not enough revenue data: set revenue, or aov together with conversions, or aov \
             with conversion_rate and cpc to forecast a campaign that has not run yet"
                .into(),
        );
    };
    if revenue <= 0.0 {
        return Err("revenue must be greater than 0".into());
    }

    let aov_resolved = if aov_in > 0.0 {
        Some(aov_in)
    } else if conversions_resolved > 0.0 {
        Some(revenue / conversions_resolved)
    } else {
        None
    };

    // ---- contribution margin ------------------------------------------------
    let (cm_percent, cm_per_order, costs) = if per_order_basis {
        let a = aov_resolved.ok_or_else(|| {
            "margin_basis 'per_order' needs an average order value: set aov, or set \
             conversions so it can be derived from revenue"
                .to_string()
        })?;
        let processing = a * payment_rate / 100.0 + payment_fixed;
        let refund_allowance = a * refund_rate / 100.0;
        let total =
            cogs_per_order + shipping_per_order + processing + refund_allowance + other_cost_per_order;
        let cm = a - total;
        if cm <= 0.0 {
            return Err(format!(
                "per-order costs come to {total:.2} against an average order value of {a:.2}, \
                 leaving no contribution margin — lower the costs or raise the order value"
            ));
        }
        (
            cm / a * 100.0,
            Some(cm),
            Some(CostBreakdown {
                cogs: cogs_per_order,
                shipping: shipping_per_order,
                processing,
                refund_allowance,
                other: other_cost_per_order,
                total,
            }),
        )
    } else {
        if gross_margin <= 0.0 {
            return Err(
                "gross_margin must be greater than 0 — with no margin on revenue, no amount \
                 of ad spend can pay for itself"
                    .into(),
            );
        }
        (gross_margin, aov_resolved.map(|a| a * gross_margin / 100.0), None)
    };
    let cm = cm_percent / 100.0;

    // ---- the ROAS family -----------------------------------------------------
    let gross_roas = revenue / ad_spend;
    let roas_percent = gross_roas * 100.0;
    let acos = ad_spend / revenue * 100.0;
    let cost_per_revenue = ad_spend / revenue;
    let net_roas = revenue * cm / ad_spend;
    let break_even_roas = 1.0 / cm;

    let target_roas = if target_net_margin > 0.0 {
        let t = target_net_margin / 100.0;
        if t >= cm - EPS {
            return Err(format!(
                "target_net_margin of {target_net_margin:.2}% is not reachable on a contribution \
                 margin of {cm_percent:.2}% — the target must stay below the margin"
            ));
        }
        Some(1.0 / (cm - t))
    } else {
        None
    };

    let ltv_roas = if purchases_per_customer > 1.0 {
        Some(revenue * purchases_per_customer * cm / ad_spend)
    } else {
        None
    };

    // ---- profit --------------------------------------------------------------
    let gross_profit = revenue * cm;
    let profit_after_ads = gross_profit - ad_spend;
    let net_profit = profit_after_ads - fixed_costs;
    let net_margin = net_profit / revenue * 100.0;
    let profit_per_spend = profit_after_ads / ad_spend;
    let break_even_spend = gross_profit;
    let break_even_revenue = ad_spend / cm;
    let headroom_percent = (gross_roas - break_even_roas) / break_even_roas * 100.0;

    let verdict = if gross_roas > break_even_roas + EPS {
        format!(
            "Above break-even: every unit of ad spend returns {:.2} of contribution margin after \
             paying for itself, with {:.1}% of headroom before the campaign stops paying.",
            profit_per_spend, headroom_percent
        )
    } else if gross_roas < break_even_roas - EPS {
        format!(
            "Below break-even: ads lose {:.2} per unit of spend. Revenue would need to reach a \
             {:.2}x return (or the margin would need to rise) to cover the spend.",
            -profit_per_spend, break_even_roas
        )
    } else {
        "Exactly at break-even: ad-driven margin covers the ad spend and nothing more.".to_string()
    };

    // ---- per-customer economics ----------------------------------------------
    let customer = if conversions_resolved > 0.0 {
        let cac = ad_spend / conversions_resolved;
        let max_cac_at_target = match (cm_per_order, aov_resolved) {
            (Some(c), Some(a)) if target_net_margin > 0.0 => Some(c - a * target_net_margin / 100.0),
            _ => None,
        };
        let ltv_revenue = aov_resolved.map(|a| a * purchases_per_customer);
        let ltv_margin = ltv_revenue.map(|r| r * cm);
        let purchases_to_repay = cm_per_order.map(|c| cac / c);
        Some(Customer {
            conversions: conversions_resolved,
            cac,
            max_cac: cm_per_order,
            max_cac_at_target,
            cac_headroom: cm_per_order.map(|c| c - cac),
            purchases_per_customer,
            ltv_revenue,
            ltv_margin,
            ltv_cac: ltv_margin.map(|m| m / cac),
            purchases_to_repay,
            payback_months: if purchase_interval_months > 0.0 {
                purchases_to_repay.map(|p| p * purchase_interval_months)
            } else {
                None
            },
        })
    } else {
        None
    };

    // ---- traffic --------------------------------------------------------------
    let clicks_block = if clicks_resolved > 0.0 {
        let cpc_resolved = if cpc_in > 0.0 {
            cpc_in
        } else {
            ad_spend / clicks_resolved
        };
        let cr = if conversions_resolved > 0.0 {
            conversions_resolved / clicks_resolved * 100.0
        } else {
            conversion_rate_in
        };
        Some(Clicks {
            clicks: clicks_resolved,
            cpc: cpc_resolved,
            conversion_rate: cr,
            clicks_per_order: if conversions_resolved > 0.0 {
                Some(clicks_resolved / conversions_resolved)
            } else {
                None
            },
            revenue_per_click: revenue / clicks_resolved,
            margin_per_click: gross_profit / clicks_resolved,
            break_even_cpc: if cr > 0.0 {
                cm_per_order.map(|c| c * cr / 100.0)
            } else {
                None
            },
        })
    } else {
        None
    };

    // ---- revenue goal ----------------------------------------------------------
    let goal = if revenue_goal > 0.0 {
        let budget_at_current = revenue_goal / gross_roas;
        Some(Goal {
            revenue_goal,
            extra_revenue: revenue_goal - revenue,
            orders_needed: aov_resolved.map(|a| revenue_goal / a),
            budget_at_current,
            extra_spend: budget_at_current - ad_spend,
            budget_at_break_even: revenue_goal / break_even_roas,
            budget_at_target: target_roas.map(|t| revenue_goal / t),
            profit_at_goal: revenue_goal * cm - budget_at_current - fixed_costs,
        })
    } else {
        None
    };

    // ---- what-if ladder ---------------------------------------------------------
    let mut scenario_rows = Vec::new();
    if scenarios {
        let mut pts: Vec<(f64, Vec<String>)> = Vec::new();
        for v in [1.0_f64, 1.5, 2.0, 2.5, 3.0, 4.0] {
            add_point(&mut pts, v, None);
        }
        add_point(&mut pts, break_even_roas, Some("break-even"));
        add_point(&mut pts, gross_roas, Some("current"));
        if let Some(t) = target_roas {
            add_point(&mut pts, t, Some("target"));
        }
        pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        for (roas, notes) in pts {
            let profit_per_spend = roas * cm - 1.0;
            let ad_cost_per_order = aov_resolved.map(|a| a / roas);
            scenario_rows.push(Scenario {
                roas,
                note: notes.join(" / "),
                profit_per_spend,
                ad_cost_per_order,
                profit_per_order: match (cm_per_order, ad_cost_per_order) {
                    (Some(c), Some(cost)) => Some(c - cost),
                    _ => None,
                },
                net_profit: ad_spend * profit_per_spend - fixed_costs,
            });
        }
    }

    Ok(Report {
        ad_spend,
        revenue,
        aov: aov_resolved,
        margin_basis: if per_order_basis {
            "per_order".into()
        } else {
            "percent".into()
        },
        cm_percent,
        cm_per_order,
        costs,
        gross_roas,
        roas_percent,
        acos,
        cost_per_revenue,
        net_roas,
        break_even_roas,
        target_net_margin,
        target_roas,
        ltv_roas,
        gross_profit,
        profit_after_ads,
        fixed_costs,
        net_profit,
        net_margin,
        profit_per_spend,
        break_even_spend,
        break_even_revenue,
        headroom_percent,
        verdict,
        customer,
        clicks: clicks_block,
        goal,
        scenarios: scenario_rows,
    })
}

// ---------------------------------------------------------------------------
// formatting
// ---------------------------------------------------------------------------

/// Round half away from zero to `dp` decimal places.
fn round_dp(v: f64, dp: usize) -> f64 {
    let f = 10f64.powi(dp as i32);
    (v * f).round() / f
}

/// Format money with thousands separators and a currency prefix.
fn money_str(v: f64, currency: &str, dp: usize) -> String {
    let rounded = round_dp(v, dp);
    let neg = rounded < 0.0;
    let abs = rounded.abs();
    let text = format!("{abs:.dp$}");
    let (int_part, frac_part) = match text.split_once('.') {
        Some((i, f)) => (i.to_string(), Some(f.to_string())),
        None => (text, None),
    };
    let mut grouped = String::new();
    for (i, ch) in int_part.chars().enumerate() {
        if i > 0 && (int_part.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let body = match frac_part {
        Some(f) => format!("{grouped}.{f}"),
        None => grouped,
    };
    format!("{}{}{}", if neg { "-" } else { "" }, currency, body)
}

/// Format a plain number with thousands separators.
fn num_str(v: f64, dp: usize) -> String {
    money_str(v, "", dp)
}

/// Format a ROAS multiple, e.g. `3.50x`.
fn x_str(v: f64) -> String {
    format!("{}x", num_str(v, 2))
}

/// Format a percentage, e.g. `41.25%`.
fn pct_str(v: f64, dp: usize) -> String {
    format!("{}%", num_str(v, dp))
}

/// One rendered section: either label/value rows or a real multi-column table.
enum Block {
    Kv(String, Vec<(String, String)>),
    Table(String, Vec<String>, Vec<Vec<String>>),
}

/// Build every section once; the markdown, text and CSV renderers all read this.
fn blocks(r: &Report, c: &str, dp: usize) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();

    let mut headline = vec![
        ("ROAS".into(), x_str(r.gross_roas)),
        ("ROAS as a percentage".into(), pct_str(r.roas_percent, dp)),
        ("Break-even ROAS".into(), x_str(r.break_even_roas)),
        ("Net ROAS (margin per unit spent)".into(), x_str(r.net_roas)),
        ("ACOS (spend ÷ revenue)".into(), pct_str(r.acos, dp)),
    ];
    if let Some(t) = r.target_roas {
        headline.push((
            format!("Target ROAS at {} net margin", pct_str(r.target_net_margin, dp)),
            x_str(t),
        ));
    }
    if let Some(l) = r.ltv_roas {
        headline.push(("LTV-adjusted ROAS".into(), x_str(l)));
    }
    headline.push(("Verdict".into(), r.verdict.clone()));
    out.push(Block::Kv("Return on ad spend".into(), headline));

    out.push(Block::Kv(
        "Profit on the ad-driven revenue".into(),
        vec![
            ("Attributed revenue".into(), money_str(r.revenue, c, dp)),
            ("Ad spend".into(), money_str(r.ad_spend, c, dp)),
            (
                "Contribution margin".into(),
                pct_str(r.cm_percent, dp),
            ),
            ("Margin on that revenue".into(), money_str(r.gross_profit, c, dp)),
            ("Profit after ad spend".into(), money_str(r.profit_after_ads, c, dp)),
            ("Other fixed costs".into(), money_str(r.fixed_costs, c, dp)),
            ("Net profit".into(), money_str(r.net_profit, c, dp)),
            ("Net margin".into(), pct_str(r.net_margin, dp)),
            (
                "Profit per 1 of ad spend".into(),
                money_str(r.profit_per_spend, c, dp),
            ),
            (
                "Ad cost per 1 of revenue".into(),
                money_str(r.cost_per_revenue, c, dp),
            ),
            (
                "Break-even ad spend (most you could spend)".into(),
                money_str(r.break_even_spend, c, dp),
            ),
            (
                "Break-even revenue (least this spend must return)".into(),
                money_str(r.break_even_revenue, c, dp),
            ),
            (
                "Headroom above break-even ROAS".into(),
                pct_str(r.headroom_percent, dp),
            ),
        ],
    ));

    if let Some(aov) = r.aov {
        let mut rows = vec![("Average order value".into(), money_str(aov, c, dp))];
        if let Some(costs) = &r.costs {
            rows.push(("Cost of goods".into(), money_str(costs.cogs, c, dp)));
            rows.push(("Shipping and fulfilment".into(), money_str(costs.shipping, c, dp)));
            rows.push(("Payment processing".into(), money_str(costs.processing, c, dp)));
            rows.push((
                "Refund allowance".into(),
                money_str(costs.refund_allowance, c, dp),
            ));
            rows.push(("Other variable cost".into(), money_str(costs.other, c, dp)));
            rows.push(("Total variable cost".into(), money_str(costs.total, c, dp)));
        }
        if let Some(cmo) = r.cm_per_order {
            rows.push(("Contribution margin per order".into(), money_str(cmo, c, dp)));
        }
        out.push(Block::Kv("Per order".into(), rows));
    }

    if let Some(cu) = &r.customer {
        let mut rows = vec![
            ("Orders / customers from ads".into(), num_str(cu.conversions, 2)),
            ("CAC (cost to acquire one)".into(), money_str(cu.cac, c, dp)),
        ];
        if let Some(m) = cu.max_cac {
            rows.push((
                "Most you can pay per order at break-even".into(),
                money_str(m, c, dp),
            ));
        }
        if let Some(m) = cu.max_cac_at_target {
            rows.push((
                format!("Most you can pay at {} net margin", pct_str(r.target_net_margin, dp)),
                money_str(m, c, dp),
            ));
        }
        if let Some(h) = cu.cac_headroom {
            rows.push(("CAC headroom".into(), money_str(h, c, dp)));
        }
        rows.push((
            "Lifetime purchases per customer".into(),
            num_str(cu.purchases_per_customer, 2),
        ));
        if let Some(v) = cu.ltv_revenue {
            rows.push(("LTV (revenue)".into(), money_str(v, c, dp)));
        }
        if let Some(v) = cu.ltv_margin {
            rows.push(("LTV (contribution margin)".into(), money_str(v, c, dp)));
        }
        if let Some(v) = cu.ltv_cac {
            rows.push(("LTV:CAC ratio".into(), format!("{}:1", num_str(v, 2))));
        }
        if let Some(v) = cu.purchases_to_repay {
            rows.push(("Purchases to repay CAC".into(), num_str(v, 2)));
        }
        if let Some(v) = cu.payback_months {
            rows.push(("CAC payback".into(), format!("{} months", num_str(v, 2))));
        }
        out.push(Block::Kv("Customer economics".into(), rows));
    }

    if let Some(cl) = &r.clicks {
        let mut rows = vec![
            ("Clicks".into(), num_str(cl.clicks, 2)),
            ("Cost per click".into(), money_str(cl.cpc, c, dp)),
            ("Conversion rate".into(), pct_str(cl.conversion_rate, dp)),
        ];
        if let Some(v) = cl.clicks_per_order {
            rows.push(("Clicks per order".into(), num_str(v, 2)));
        }
        rows.push((
            "Revenue per click".into(),
            money_str(cl.revenue_per_click, c, dp),
        ));
        rows.push((
            "Margin per click".into(),
            money_str(cl.margin_per_click, c, dp),
        ));
        if let Some(v) = cl.break_even_cpc {
            rows.push(("Break-even cost per click".into(), money_str(v, c, dp)));
        }
        out.push(Block::Kv("Clicks and traffic".into(), rows));
    }

    if let Some(g) = &r.goal {
        let mut rows = vec![
            ("Revenue goal".into(), money_str(g.revenue_goal, c, dp)),
            ("Extra revenue needed".into(), money_str(g.extra_revenue, c, dp)),
        ];
        if let Some(v) = g.orders_needed {
            rows.push(("Orders needed".into(), num_str(v, 2)));
        }
        rows.push((
            "Budget at the current ROAS".into(),
            money_str(g.budget_at_current, c, dp),
        ));
        rows.push(("Extra ad spend needed".into(), money_str(g.extra_spend, c, dp)));
        rows.push((
            "Budget ceiling at break-even ROAS".into(),
            money_str(g.budget_at_break_even, c, dp),
        ));
        if let Some(v) = g.budget_at_target {
            rows.push(("Budget at the target ROAS".into(), money_str(v, c, dp)));
        }
        rows.push((
            "Net profit at the goal".into(),
            money_str(g.profit_at_goal, c, dp),
        ));
        out.push(Block::Kv("Reaching a revenue goal".into(), rows));
    }

    if !r.scenarios.is_empty() {
        let per_order = r.scenarios.iter().any(|s| s.profit_per_order.is_some());
        let mut headers = vec!["ROAS".to_string(), "Profit per 1 spent".to_string()];
        if per_order {
            headers.push("Ad cost per order".to_string());
            headers.push("Profit per order".to_string());
        }
        headers.push("Net profit".to_string());
        headers.push("Note".to_string());
        let rows: Vec<Vec<String>> = r
            .scenarios
            .iter()
            .map(|s| {
                let mut row = vec![x_str(s.roas), money_str(s.profit_per_spend, c, dp)];
                if per_order {
                    row.push(
                        s.ad_cost_per_order
                            .map(|v| money_str(v, c, dp))
                            .unwrap_or_else(|| "-".into()),
                    );
                    row.push(
                        s.profit_per_order
                            .map(|v| money_str(v, c, dp))
                            .unwrap_or_else(|| "-".into()),
                    );
                }
                row.push(money_str(s.net_profit, c, dp));
                row.push(if s.note.is_empty() {
                    "-".into()
                } else {
                    s.note.clone()
                });
                row
            })
            .collect();
        out.push(Block::Table(
            "What-if: profit at other ROAS levels".into(),
            headers,
            rows,
        ));
    }

    out
}

/// Markdown pipe tables, or the same content as aligned plain text.
fn render(r: &Report, c: &str, dp: usize, markdown: bool) -> String {
    let secs = blocks(r, c, dp);
    let mut s = String::new();
    for (i, b) in secs.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        match b {
            Block::Kv(title, rows) => {
                if markdown {
                    s.push_str(&format!("## {title}\n\n| Item | Value |\n| --- | --- |\n"));
                    for (k, v) in rows {
                        s.push_str(&format!("| {k} | {v} |\n"));
                    }
                } else {
                    s.push_str(&format!("{title}\n{}\n", "-".repeat(title.chars().count())));
                    let width = rows.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);
                    for (k, v) in rows {
                        s.push_str(&format!("{k:width$}  {v}\n"));
                    }
                }
            }
            Block::Table(title, headers, rows) => {
                if markdown {
                    s.push_str(&format!("## {title}\n\n"));
                    s.push_str(&format!("| {} |\n", headers.join(" | ")));
                    s.push_str(&format!(
                        "| {} |\n",
                        headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
                    ));
                    for row in rows {
                        s.push_str(&format!("| {} |\n", row.join(" | ")));
                    }
                } else {
                    s.push_str(&format!("{title}\n{}\n", "-".repeat(title.chars().count())));
                    let mut widths: Vec<usize> =
                        headers.iter().map(|h| h.chars().count()).collect();
                    for row in rows {
                        for (i, cell) in row.iter().enumerate() {
                            if i < widths.len() {
                                widths[i] = widths[i].max(cell.chars().count());
                            }
                        }
                    }
                    let line = |cells: &[String]| {
                        cells
                            .iter()
                            .enumerate()
                            .map(|(i, cell)| {
                                let w = widths.get(i).copied().unwrap_or(0);
                                format!("{cell:w$}")
                            })
                            .collect::<Vec<_>>()
                            .join("  ")
                            .trim_end()
                            .to_string()
                    };
                    s.push_str(&format!("{}\n", line(headers)));
                    for row in rows {
                        s.push_str(&format!("{}\n", line(row)));
                    }
                }
            }
        }
    }
    s.trim_end().to_string()
}

/// Escape one CSV cell.
fn esc(v: &str) -> String {
    if v.contains(',') || v.contains('"') || v.contains('\n') {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

/// `section,item,value` rows, then the what-if table as its own CSV block.
fn render_csv(r: &Report, dp: usize) -> String {
    let secs = blocks(r, "", dp);
    let mut s = String::from("section,item,value\n");
    for b in &secs {
        if let Block::Kv(title, rows) = b {
            for (k, v) in rows {
                s.push_str(&format!("{},{},{}\n", esc(title), esc(k), esc(v)));
            }
        }
    }
    for b in &secs {
        if let Block::Table(title, headers, rows) = b {
            s.push('\n');
            s.push_str(&format!("{}\n", esc(title)));
            s.push_str(&format!(
                "{}\n",
                headers.iter().map(|h| esc(h)).collect::<Vec<_>>().join(",")
            ));
            for row in rows {
                s.push_str(&format!(
                    "{}\n",
                    row.iter().map(|v| esc(v)).collect::<Vec<_>>().join(",")
                ));
            }
        }
    }
    s.trim_end().to_string()
}

/// The full result as JSON — every figure at the requested precision.
fn render_json(r: &Report, dp: usize) -> String {
    let n = |v: f64| {
        let x = round_dp(v, dp);
        if x == 0.0 {
            "0".to_string()
        } else {
            format!("{x}")
        }
    };
    let n2 = |v: f64| {
        let x = round_dp(v, 2);
        if x == 0.0 {
            "0".to_string()
        } else {
            format!("{x}")
        }
    };
    let opt = |v: Option<f64>, f: &dyn Fn(f64) -> String| match v {
        Some(x) => f(x),
        None => "null".to_string(),
    };

    let mut s = String::from("{\n");
    s.push_str(&format!("  \"ad_spend\": {},\n", n(r.ad_spend)));
    s.push_str(&format!("  \"revenue\": {},\n", n(r.revenue)));
    s.push_str(&format!("  \"aov\": {},\n", opt(r.aov, &n)));
    s.push_str(&format!("  \"margin_basis\": \"{}\",\n", r.margin_basis));
    s.push_str(&format!("  \"contribution_margin_percent\": {},\n", n2(r.cm_percent)));
    s.push_str(&format!(
        "  \"contribution_margin_per_order\": {},\n",
        opt(r.cm_per_order, &n)
    ));
    s.push_str(&format!("  \"roas\": {},\n", n2(r.gross_roas)));
    s.push_str(&format!("  \"roas_percent\": {},\n", n2(r.roas_percent)));
    s.push_str(&format!("  \"acos_percent\": {},\n", n2(r.acos)));
    s.push_str(&format!("  \"net_roas\": {},\n", n2(r.net_roas)));
    s.push_str(&format!("  \"break_even_roas\": {},\n", n2(r.break_even_roas)));
    s.push_str(&format!("  \"target_roas\": {},\n", opt(r.target_roas, &n2)));
    s.push_str(&format!("  \"ltv_roas\": {},\n", opt(r.ltv_roas, &n2)));
    s.push_str(&format!("  \"gross_profit\": {},\n", n(r.gross_profit)));
    s.push_str(&format!("  \"profit_after_ads\": {},\n", n(r.profit_after_ads)));
    s.push_str(&format!("  \"fixed_costs\": {},\n", n(r.fixed_costs)));
    s.push_str(&format!("  \"net_profit\": {},\n", n(r.net_profit)));
    s.push_str(&format!("  \"net_margin_percent\": {},\n", n2(r.net_margin)));
    s.push_str(&format!("  \"profit_per_spend\": {},\n", n2(r.profit_per_spend)));
    s.push_str(&format!("  \"cost_per_revenue\": {},\n", n2(r.cost_per_revenue)));
    s.push_str(&format!("  \"break_even_spend\": {},\n", n(r.break_even_spend)));
    s.push_str(&format!("  \"break_even_revenue\": {},\n", n(r.break_even_revenue)));
    s.push_str(&format!("  \"headroom_percent\": {},\n", n2(r.headroom_percent)));
    s.push_str(&format!("  \"verdict\": {},\n", esc_json(&r.verdict)));

    match &r.customer {
        Some(cu) => {
            s.push_str("  \"customer\": {\n");
            s.push_str(&format!("    \"conversions\": {},\n", n2(cu.conversions)));
            s.push_str(&format!("    \"cac\": {},\n", n(cu.cac)));
            s.push_str(&format!("    \"max_cac\": {},\n", opt(cu.max_cac, &n)));
            s.push_str(&format!(
                "    \"max_cac_at_target\": {},\n",
                opt(cu.max_cac_at_target, &n)
            ));
            s.push_str(&format!("    \"cac_headroom\": {},\n", opt(cu.cac_headroom, &n)));
            s.push_str(&format!(
                "    \"purchases_per_customer\": {},\n",
                n2(cu.purchases_per_customer)
            ));
            s.push_str(&format!("    \"ltv_revenue\": {},\n", opt(cu.ltv_revenue, &n)));
            s.push_str(&format!("    \"ltv_margin\": {},\n", opt(cu.ltv_margin, &n)));
            s.push_str(&format!("    \"ltv_cac\": {},\n", opt(cu.ltv_cac, &n2)));
            s.push_str(&format!(
                "    \"purchases_to_repay_cac\": {},\n",
                opt(cu.purchases_to_repay, &n2)
            ));
            s.push_str(&format!(
                "    \"payback_months\": {}\n",
                opt(cu.payback_months, &n2)
            ));
            s.push_str("  },\n");
        }
        None => s.push_str("  \"customer\": null,\n"),
    }

    match &r.clicks {
        Some(cl) => {
            s.push_str("  \"clicks\": {\n");
            s.push_str(&format!("    \"clicks\": {},\n", n2(cl.clicks)));
            s.push_str(&format!("    \"cpc\": {},\n", n(cl.cpc)));
            s.push_str(&format!(
                "    \"conversion_rate_percent\": {},\n",
                n2(cl.conversion_rate)
            ));
            s.push_str(&format!(
                "    \"clicks_per_order\": {},\n",
                opt(cl.clicks_per_order, &n2)
            ));
            s.push_str(&format!(
                "    \"revenue_per_click\": {},\n",
                n(cl.revenue_per_click)
            ));
            s.push_str(&format!("    \"margin_per_click\": {},\n", n(cl.margin_per_click)));
            s.push_str(&format!(
                "    \"break_even_cpc\": {}\n",
                opt(cl.break_even_cpc, &n)
            ));
            s.push_str("  },\n");
        }
        None => s.push_str("  \"clicks\": null,\n"),
    }

    match &r.goal {
        Some(g) => {
            s.push_str("  \"goal\": {\n");
            s.push_str(&format!("    \"revenue_goal\": {},\n", n(g.revenue_goal)));
            s.push_str(&format!("    \"extra_revenue\": {},\n", n(g.extra_revenue)));
            s.push_str(&format!("    \"orders_needed\": {},\n", opt(g.orders_needed, &n2)));
            s.push_str(&format!(
                "    \"budget_at_current_roas\": {},\n",
                n(g.budget_at_current)
            ));
            s.push_str(&format!("    \"extra_spend\": {},\n", n(g.extra_spend)));
            s.push_str(&format!(
                "    \"budget_at_break_even_roas\": {},\n",
                n(g.budget_at_break_even)
            ));
            s.push_str(&format!(
                "    \"budget_at_target_roas\": {},\n",
                opt(g.budget_at_target, &n)
            ));
            s.push_str(&format!("    \"profit_at_goal\": {}\n", n(g.profit_at_goal)));
            s.push_str("  },\n");
        }
        None => s.push_str("  \"goal\": null,\n"),
    }

    s.push_str("  \"scenarios\": [");
    for (i, sc) in r.scenarios.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("\n    {");
        s.push_str(&format!("\"roas\": {}, ", n2(sc.roas)));
        s.push_str(&format!("\"profit_per_spend\": {}, ", n2(sc.profit_per_spend)));
        s.push_str(&format!(
            "\"ad_cost_per_order\": {}, ",
            opt(sc.ad_cost_per_order, &n)
        ));
        s.push_str(&format!(
            "\"profit_per_order\": {}, ",
            opt(sc.profit_per_order, &n)
        ));
        s.push_str(&format!("\"net_profit\": {}, ", n(sc.net_profit)));
        s.push_str(&format!("\"note\": {}", esc_json(&sc.note)));
        s.push('}');
    }
    if r.scenarios.is_empty() {
        s.push_str("]\n}");
    } else {
        s.push_str("\n  ]\n}");
    }
    s
}

/// Quote + escape a string as a JSON value.
fn esc_json(v: &str) -> String {
    let mut s = String::from("\"");
    for ch in v.chars() {
        match ch {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if (c as u32) < 0x20 => s.push_str(&format!("\\u{:04x}", c as u32)),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

/// Run the calculation and render it in the requested format.
#[allow(clippy::too_many_arguments)]
pub fn run(
    ad_spend: f64,
    revenue: f64,
    conversions: f64,
    aov: f64,
    margin_basis: &str,
    gross_margin: f64,
    cogs_per_order: f64,
    shipping_per_order: f64,
    payment_rate: f64,
    payment_fixed: f64,
    refund_rate: f64,
    other_cost_per_order: f64,
    target_net_margin: f64,
    fixed_costs: f64,
    clicks: f64,
    cpc: f64,
    conversion_rate: f64,
    purchases_per_customer: f64,
    purchase_interval_months: f64,
    revenue_goal: f64,
    scenarios: bool,
    currency: &str,
    decimals: f64,
    format: &str,
) -> Result<String, String> {
    if !decimals.is_finite() || !(0.0..=6.0).contains(&decimals) {
        return Err("decimals must be a whole number between 0 and 6".into());
    }
    let dp = decimals.round() as usize;
    let currency = currency.trim();
    if currency.chars().count() > 8 {
        return Err("currency must be at most 8 characters, for example '$', '€' or 'CHF '".into());
    }
    let r = compute(
        ad_spend,
        revenue,
        conversions,
        aov,
        margin_basis,
        gross_margin,
        cogs_per_order,
        shipping_per_order,
        payment_rate,
        payment_fixed,
        refund_rate,
        other_cost_per_order,
        target_net_margin,
        fixed_costs,
        clicks,
        cpc,
        conversion_rate,
        purchases_per_customer,
        purchase_interval_months,
        revenue_goal,
        scenarios,
    )?;

    match format.trim().to_ascii_lowercase().as_str() {
        "markdown" => Ok(render(&r, currency, dp, true)),
        "text" => Ok(render(&r, currency, dp, false)),
        "csv" => Ok(render_csv(&r, dp)),
        "json" => Ok(render_json(&r, dp)),
        other => Err(format!(
            "format must be one of markdown, text, csv, json — got '{other}'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default shape: spend + revenue + a 50% gross margin, nothing optional on.
    fn basic() -> Report {
        compute(
            10_000.0, 35_000.0, 0.0, 0.0, "percent", 50.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0, false,
        )
        .unwrap()
    }

    #[test]
    fn roas_break_even_and_profit_from_spend_and_revenue() {
        let r = basic();
        assert_eq!(r.gross_roas, 3.5);
        assert_eq!(r.roas_percent, 350.0);
        assert_eq!(r.break_even_roas, 2.0);
        assert_eq!(r.net_roas, 1.75);
        // 35000 * 50% = 17500 margin, minus 10000 spend.
        assert_eq!(r.gross_profit, 17_500.0);
        assert_eq!(r.profit_after_ads, 7_500.0);
        assert_eq!(r.net_profit, 7_500.0);
        assert_eq!(r.profit_per_spend, 0.75);
        assert_eq!(r.break_even_spend, 17_500.0);
        assert_eq!(r.break_even_revenue, 20_000.0);
        assert_eq!(r.headroom_percent, 75.0);
        assert!((r.acos - 28.571_428_571_428_573).abs() < 1e-9);
        assert!(r.verdict.starts_with("Above break-even"));
        assert!(r.customer.is_none());
        assert!(r.clicks.is_none());
    }

    #[test]
    fn per_order_costs_build_the_contribution_margin() {
        // 80 AOV - 28 cogs - 7 shipping - (2.9% + 0.30) processing - 3% refunds.
        let r = compute(
            1_000.0, 0.0, 50.0, 80.0, "per_order", 0.0, 28.0, 7.0, 2.9, 0.3, 3.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0, false,
        )
        .unwrap();
        let costs = r.costs.as_ref().unwrap();
        assert!((costs.processing - 2.62).abs() < 1e-9);
        assert!((costs.refund_allowance - 2.4).abs() < 1e-9);
        assert!((r.cm_per_order.unwrap() - 39.98).abs() < 1e-9);
        assert!((r.cm_percent - 49.975).abs() < 1e-9);
        // Break-even ROAS = AOV / contribution margin per order.
        assert!((r.break_even_roas - 80.0 / 39.98).abs() < 1e-9);
        // Revenue was derived from aov x conversions.
        assert_eq!(r.revenue, 4_000.0);
        assert_eq!(r.aov, Some(80.0));
    }

    #[test]
    fn cac_ltv_and_payback_come_from_the_order_count() {
        let r = compute(
            1_000.0, 4_000.0, 50.0, 0.0, "percent", 40.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 3.0, 2.0, 0.0, false,
        )
        .unwrap();
        let cu = r.customer.as_ref().unwrap();
        assert_eq!(r.aov, Some(80.0));
        assert_eq!(cu.cac, 20.0);
        // 40% of an 80 order = 32 of margin, so 32 is the most an order can cost.
        assert_eq!(cu.max_cac, Some(32.0));
        assert_eq!(cu.cac_headroom, Some(12.0));
        assert_eq!(cu.ltv_revenue, Some(240.0));
        assert_eq!(cu.ltv_margin, Some(96.0));
        assert_eq!(cu.ltv_cac, Some(4.8));
        assert_eq!(cu.purchases_to_repay, Some(0.625));
        assert_eq!(cu.payback_months, Some(1.25));
        // 3 lifetime purchases lift the LTV-adjusted ROAS to 3 x the net ROAS.
        assert_eq!(r.ltv_roas, Some(4.8));
    }

    #[test]
    fn forecast_mode_derives_everything_from_cpc_and_conversion_rate() {
        // 1000 spend at 2.00 CPC = 500 clicks; 4% of them convert at 100 AOV.
        let r = compute(
            1_000.0, 0.0, 0.0, 100.0, "percent", 60.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0,
            2.0, 4.0, 1.0, 0.0, 0.0, false,
        )
        .unwrap();
        let cl = r.clicks.as_ref().unwrap();
        assert_eq!(cl.clicks, 500.0);
        assert_eq!(cl.cpc, 2.0);
        assert_eq!(cl.conversion_rate, 4.0);
        assert_eq!(cl.clicks_per_order, Some(25.0));
        assert_eq!(r.revenue, 2_000.0);
        assert_eq!(r.gross_roas, 2.0);
        assert_eq!(cl.revenue_per_click, 4.0);
        // Break-even CPC = margin per order x conversion rate = 60 x 0.04.
        assert_eq!(cl.break_even_cpc, Some(2.4));
        assert_eq!(r.customer.as_ref().unwrap().cac, 50.0);
    }

    #[test]
    fn target_net_margin_sets_the_target_roas_and_max_cac() {
        let r = compute(
            10_000.0, 35_000.0, 350.0, 0.0, "percent", 50.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 15.0,
            0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, false,
        )
        .unwrap();
        // 1 / (0.50 - 0.15) = 2.857...
        assert!((r.target_roas.unwrap() - 1.0 / 0.35).abs() < 1e-9);
        let cu = r.customer.as_ref().unwrap();
        assert_eq!(r.aov, Some(100.0));
        // 50 of margin per order, less 15% of the 100 order value.
        assert_eq!(cu.max_cac_at_target, Some(35.0));
    }

    #[test]
    fn revenue_goal_gives_the_budget_at_each_roas() {
        let r = compute(
            10_000.0, 35_000.0, 0.0, 0.0, "percent", 50.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0,
            2_000.0, 0.0, 0.0, 0.0, 1.0, 0.0, 70_000.0, false,
        )
        .unwrap();
        let g = r.goal.as_ref().unwrap();
        assert_eq!(g.extra_revenue, 35_000.0);
        assert_eq!(g.budget_at_current, 20_000.0);
        assert_eq!(g.extra_spend, 10_000.0);
        assert_eq!(g.budget_at_break_even, 35_000.0);
        // 70000 * 50% - 20000 spend - 2000 fixed.
        assert_eq!(g.profit_at_goal, 13_000.0);
        assert_eq!(r.net_profit, 5_500.0);
    }

    #[test]
    fn scenario_ladder_marks_break_even_current_and_target() {
        let r = compute(
            10_000.0, 35_000.0, 350.0, 0.0, "percent", 50.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 15.0,
            0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, true,
        )
        .unwrap();
        assert!(!r.scenarios.is_empty());
        let be = r.scenarios.iter().find(|s| s.note.contains("break-even")).unwrap();
        assert_eq!(be.roas, 2.0);
        assert!(be.profit_per_spend.abs() < 1e-9);
        assert!(be.net_profit.abs() < 1e-9);
        // At break-even the ad cost per order exactly equals the margin per order.
        assert_eq!(be.ad_cost_per_order, Some(50.0));
        assert!(be.profit_per_order.unwrap().abs() < 1e-9);
        let cur = r.scenarios.iter().find(|s| s.note.contains("current")).unwrap();
        assert_eq!(cur.roas, 3.5);
        assert_eq!(cur.net_profit, 7_500.0);
        assert!(r.scenarios.iter().any(|s| s.note.contains("target")));
        // Sorted ascending, no duplicate rungs.
        for w in r.scenarios.windows(2) {
            assert!(w[0].roas < w[1].roas);
        }
    }

    #[test]
    fn below_break_even_is_reported_as_a_loss() {
        let r = compute(
            10_000.0, 15_000.0, 0.0, 0.0, "percent", 50.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0, false,
        )
        .unwrap();
        assert_eq!(r.profit_after_ads, -2_500.0);
        assert!(r.headroom_percent < 0.0);
        assert!(r.verdict.starts_with("Below break-even"));
    }

    #[test]
    fn markdown_text_csv_and_json_all_render() {
        let args = || {
            (
                10_000.0, 35_000.0, 350.0, 0.0, 50.0_f64, 15.0_f64,
            )
        };
        let (spend, rev, conv, aov, gm, tgt) = args();
        // Every renderer must carry the same 3.50x ROAS, spelled its own way.
        let matchers: Vec<(&str, Box<dyn Fn(&str) -> bool>)> = vec![
            (
                "markdown",
                Box::new(|o: &str| o.contains("| ROAS | 3.50x |")) as Box<dyn Fn(&str) -> bool>,
            ),
            (
                "text",
                Box::new(|o: &str| {
                    o.lines()
                        .any(|l| l.starts_with("ROAS ") && l.ends_with("  3.50x"))
                }),
            ),
            (
                "csv",
                Box::new(|o: &str| o.contains("Return on ad spend,ROAS,3.50x")),
            ),
            ("json", Box::new(|o: &str| o.contains("\"roas\": 3.5,"))),
        ];
        for (f, ok) in matchers {
            let out = run(
                spend, rev, conv, aov, "percent", gm, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, tgt, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0, 0.0, true, "$", 2.0, f,
            )
            .unwrap();
            assert!(ok(&out), "{f} output missing the 3.50x ROAS:\n{out}");
        }
        let md = run(
            spend, rev, conv, aov, "percent", gm, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, tgt, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0, false, "$", 2.0, "markdown",
        )
        .unwrap();
        assert!(md.starts_with("## Return on ad spend"));
        assert!(md.contains("| Break-even ROAS | 2.00x |"));
        assert!(!md.contains("What-if"), "scenarios=false must drop the ladder");
    }

    #[test]
    fn json_output_is_parseable_shaped_and_honours_decimals() {
        let out = run(
            10_000.0, 35_000.0, 350.0, 0.0, "percent", 50.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, false, "$", 0.0, "json",
        )
        .unwrap();
        assert!(out.contains("\"roas\": 3.5"));
        assert!(out.contains("\"break_even_roas\": 2"));
        assert!(out.contains("\"gross_profit\": 17500"));
        assert!(out.contains("\"clicks\": null"));
        assert!(out.contains("\"scenarios\": []"));
    }

    #[test]
    fn currency_prefix_is_display_only() {
        let out = run(
            10_000.0, 35_000.0, 0.0, 0.0, "percent", 50.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0, false, "€", 2.0, "text",
        )
        .unwrap();
        assert!(out.contains("€35,000.00"));
    }

    // ---- error paths -------------------------------------------------------

    #[test]
    fn zero_ad_spend_is_an_error() {
        let err = compute(
            0.0, 100.0, 0.0, 0.0, "percent", 50.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0, 0.0, false,
        )
        .unwrap_err();
        assert!(err.contains("ad_spend must be greater than 0"), "{err}");
    }

    #[test]
    fn missing_revenue_data_explains_what_to_provide() {
        let err = compute(
            100.0, 0.0, 0.0, 0.0, "percent", 50.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0, 0.0, false,
        )
        .unwrap_err();
        assert!(err.contains("not enough revenue data"), "{err}");
        assert!(err.contains("aov"), "{err}");
    }

    #[test]
    fn per_order_costs_above_the_order_value_are_rejected() {
        let err = compute(
            100.0, 0.0, 10.0, 50.0, "per_order", 0.0, 45.0, 10.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0, false,
        )
        .unwrap_err();
        assert!(err.contains("no contribution margin"), "{err}");
    }

    #[test]
    fn a_target_margin_above_the_contribution_margin_is_rejected() {
        let err = compute(
            100.0, 300.0, 0.0, 0.0, "percent", 20.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 30.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0, 0.0, false,
        )
        .unwrap_err();
        assert!(err.contains("not reachable"), "{err}");
    }

    #[test]
    fn unknown_enum_values_are_rejected() {
        let err = compute(
            100.0, 300.0, 0.0, 0.0, "blended", 20.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0, 0.0, false,
        )
        .unwrap_err();
        assert!(err.contains("margin_basis must be"), "{err}");

        let err = run(
            100.0, 300.0, 0.0, 0.0, "percent", 20.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0, 0.0, false, "$", 2.0, "yaml",
        )
        .unwrap_err();
        assert!(err.contains("format must be one of"), "{err}");
    }

    #[test]
    fn out_of_range_percentages_are_rejected() {
        let err = compute(
            100.0, 300.0, 0.0, 0.0, "percent", 140.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0, 0.0, false,
        )
        .unwrap_err();
        assert!(err.contains("gross_margin must be between 0 and 100"), "{err}");

        let err = run(
            100.0, 300.0, 0.0, 0.0, "percent", 20.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0, 0.0, false, "$", 9.0, "text",
        )
        .unwrap_err();
        assert!(err.contains("decimals"), "{err}");
    }

    #[test]
    fn a_zero_gross_margin_cannot_break_even() {
        let err = compute(
            100.0, 300.0, 0.0, 0.0, "percent", 0.0, 0.0, 0.0, 2.9, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0, 0.0, false,
        )
        .unwrap_err();
        assert!(err.contains("gross_margin must be greater than 0"), "{err}");
    }
}
