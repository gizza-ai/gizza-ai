//! freelance-rate-calc core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Works backwards from a take-home income goal to the hourly/day rate a
//! self-employed person has to charge: annual costs are added, tax is grossed
//! up (never a naive `net * (1 + rate)`), paid time off and non-billable time
//! shrink the year down to real billable hours, and an optional buffer margin
//! is applied as a markup on the resulting rate.

/// US self-employment tax rate (Social Security 12.4% + Medicare 2.9%).
const SE_TAX_RATE: f64 = 0.153;
/// Share of net profit the US self-employment tax is actually assessed on.
const SE_TAX_BASE: f64 = 0.9235;

/// Everything the calculation produced, in one flat record.
#[derive(Debug, Clone, PartialEq)]
pub struct Rates {
    // time budget
    pub weeks_available: f64,
    pub work_days: f64,
    pub work_hours: f64,
    pub billable_hours: f64,
    pub billable_days: f64,
    // money
    pub target_income: f64,
    pub business_expenses: f64,
    pub health_insurance: f64,
    pub retirement: f64,
    pub total_costs: f64,
    pub pre_tax_need: f64,
    pub income_tax: f64,
    pub self_employment_tax: f64,
    pub total_tax: f64,
    pub required_revenue: f64,
    pub effective_tax_percent: f64,
    // rates
    pub base_hourly: f64,
    pub buffer_amount: f64,
    pub hourly_rate: f64,
    pub day_rate: f64,
    pub week_rate: f64,
    pub month_rate: f64,
    pub worked_hour_rate: f64,
    // optional extras
    pub current_rate: Option<CurrentRate>,
    pub quote: Option<Quote>,
}

/// Comparison of a rate the user already charges against the required rate.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentRate {
    pub rate: f64,
    pub revenue: f64,
    pub take_home: f64,
    pub rate_gap: f64,
    pub income_gap: f64,
    pub percent_of_required: f64,
}

/// A fixed-price project quote derived from the computed hourly rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub hours: f64,
    pub label: String,
    pub multiplier: f64,
    pub base: f64,
    pub adjustment: f64,
    pub total: f64,
    pub deposit: f64,
    pub midpoint: f64,
    pub final_payment: f64,
    pub days: f64,
}

/// Compute every figure. All money arguments are annual amounts in one currency.
///
/// - `target_income`    — the take-home pay the year has to produce, after tax and costs.
/// - `business_expenses`, `health_insurance`, `retirement` — annual costs, each ≥ 0.
/// - `tax_rate`         — effective income-tax percentage on profit, 0…95.
/// - `tax_basis`        — `income_only` or `self_employment` (adds the US 15.3% layer).
/// - `hours_per_week`, `days_per_week`, `hours_per_day` — the shape of a working week.
/// - `weeks_per_year`   — calendar weeks in the working year (52 unless you have a reason).
/// - `vacation_weeks`, `holidays`, `sick_days` — unpaid time off.
/// - `billable_percent` — share of worked hours a client is actually invoiced for.
/// - `buffer_percent`   — markup on the final rate for slow months / risk.
#[allow(clippy::too_many_arguments)]
pub fn compute(
    target_income: f64,
    business_expenses: f64,
    health_insurance: f64,
    retirement: f64,
    tax_rate: f64,
    tax_basis: &str,
    hours_per_week: f64,
    days_per_week: f64,
    hours_per_day: f64,
    weeks_per_year: f64,
    vacation_weeks: f64,
    holidays: f64,
    sick_days: f64,
    billable_percent: f64,
    buffer_percent: f64,
    current_rate: f64,
    project_hours: f64,
    complexity: &str,
) -> Result<Rates, String> {
    let money = |v: f64, name: &str| -> Result<f64, String> {
        if !v.is_finite() {
            return Err(format!("{name} must be a finite number"));
        }
        if v < 0.0 {
            return Err(format!("{name} must be zero or positive"));
        }
        if v > 1e12 {
            return Err(format!("{name} must be at most 1000000000000"));
        }
        Ok(v)
    };

    let target_income = money(target_income, "target_income")?;
    if target_income <= 0.0 {
        return Err("target_income must be greater than 0".into());
    }
    let business_expenses = money(business_expenses, "business_expenses")?;
    let health_insurance = money(health_insurance, "health_insurance")?;
    let retirement = money(retirement, "retirement")?;

    if !tax_rate.is_finite() || !(0.0..=95.0).contains(&tax_rate) {
        return Err("tax_rate must be between 0 and 95 percent".into());
    }
    let basis = tax_basis.trim().to_ascii_lowercase();
    let se_tax_on = match basis.as_str() {
        "income_only" => false,
        "self_employment" => true,
        other => {
            return Err(format!(
                "tax_basis must be 'income_only' or 'self_employment', got '{other}'"
            ))
        }
    };

    if !hours_per_week.is_finite() || hours_per_week <= 0.0 || hours_per_week > 168.0 {
        return Err("hours_per_week must be greater than 0 and at most 168".into());
    }
    if !days_per_week.is_finite() || days_per_week <= 0.0 || days_per_week > 7.0 {
        return Err("days_per_week must be greater than 0 and at most 7".into());
    }
    if !hours_per_day.is_finite() || hours_per_day <= 0.0 || hours_per_day > 24.0 {
        return Err("hours_per_day must be greater than 0 and at most 24".into());
    }
    if !weeks_per_year.is_finite() || weeks_per_year <= 0.0 || weeks_per_year > 53.0 {
        return Err("weeks_per_year must be greater than 0 and at most 53".into());
    }
    if !vacation_weeks.is_finite() || vacation_weeks < 0.0 {
        return Err("vacation_weeks must be zero or positive".into());
    }
    if !holidays.is_finite() || holidays < 0.0 {
        return Err("holidays must be zero or positive".into());
    }
    if !sick_days.is_finite() || sick_days < 0.0 {
        return Err("sick_days must be zero or positive".into());
    }
    if !billable_percent.is_finite() || !(1.0..=100.0).contains(&billable_percent) {
        return Err("billable_percent must be between 1 and 100".into());
    }
    if !buffer_percent.is_finite() || !(0.0..=200.0).contains(&buffer_percent) {
        return Err("buffer_percent must be between 0 and 200".into());
    }
    if !current_rate.is_finite() || current_rate < 0.0 {
        return Err("current_rate must be zero or positive".into());
    }
    if !project_hours.is_finite() || project_hours < 0.0 || project_hours > 100_000.0 {
        return Err("project_hours must be between 0 and 100000".into());
    }

    // ---- time budget -------------------------------------------------------
    // Days off (holidays + sick days) are converted to weeks at the user's own
    // days_per_week so a 4-day week loses the right share of the year.
    let days_off_as_weeks = (holidays + sick_days) / days_per_week;
    let weeks_available = weeks_per_year - vacation_weeks - days_off_as_weeks;
    if weeks_available <= 0.0 {
        return Err(format!(
            "no working weeks left: {vacation_weeks} vacation weeks plus {holidays} holidays and \
             {sick_days} sick days use up the whole {weeks_per_year}-week year"
        ));
    }
    let work_days = weeks_available * days_per_week;
    let work_hours = weeks_available * hours_per_week;
    let billable_hours = work_hours * billable_percent / 100.0;
    if billable_hours < 1.0 {
        return Err("fewer than 1 billable hour per year — raise the hours, the weeks or the billable percentage".into());
    }
    let billable_days = billable_hours / hours_per_day;

    // ---- money -------------------------------------------------------------
    let total_costs = business_expenses + health_insurance + retirement;
    // Profit that must survive tax: the take-home goal (costs are deducted
    // before tax, so they are added back after the gross-up, not inside it).
    let pre_tax_need = target_income;
    let rate = tax_rate / 100.0;
    // Gross-up plus, optionally, the US self-employment layer. Both taxes are
    // assessed on the same net profit P, so:
    //   P - P*rate - P*SE_TAX_BASE*SE_TAX_RATE = target_income
    let se_component = if se_tax_on {
        SE_TAX_BASE * SE_TAX_RATE
    } else {
        0.0
    };
    let keep = 1.0 - rate - se_component;
    if keep <= 0.0 {
        return Err(
            "the tax rate leaves nothing to take home — lower tax_rate or turn off the \
             self-employment tax layer"
                .into(),
        );
    }
    let profit = pre_tax_need / keep;
    let income_tax = profit * rate;
    let self_employment_tax = profit * se_component;
    let total_tax = income_tax + self_employment_tax;
    let required_revenue = profit + total_costs;
    let effective_tax_percent = if profit > 0.0 {
        total_tax / profit * 100.0
    } else {
        0.0
    };

    // ---- rates -------------------------------------------------------------
    let base_hourly = required_revenue / billable_hours;
    let hourly_rate = base_hourly * (1.0 + buffer_percent / 100.0);
    let buffer_amount = hourly_rate - base_hourly;
    let day_rate = hourly_rate * hours_per_day;
    let week_rate = hourly_rate * billable_hours / weeks_available;
    let month_rate = hourly_rate * billable_hours / 12.0;
    let worked_hour_rate = hourly_rate * billable_hours / work_hours;

    // ---- optional: compare a rate already being charged --------------------
    let current = if current_rate > 0.0 {
        let revenue = current_rate * billable_hours;
        // Reverse the same model: costs off the top, then both tax layers.
        let profit_now = revenue - total_costs;
        let take_home = if profit_now <= 0.0 {
            profit_now
        } else {
            profit_now * keep
        };
        Some(CurrentRate {
            rate: current_rate,
            revenue,
            take_home,
            rate_gap: hourly_rate - current_rate,
            income_gap: take_home - target_income,
            percent_of_required: if hourly_rate > 0.0 {
                current_rate / hourly_rate * 100.0
            } else {
                0.0
            },
        })
    } else {
        None
    };

    // ---- optional: fixed-price project quote -------------------------------
    let quote = if project_hours > 0.0 {
        let (label, multiplier) = complexity_multiplier(complexity)?;
        let base = hourly_rate * project_hours;
        let total = base * multiplier;
        Some(Quote {
            hours: project_hours,
            label,
            multiplier,
            base,
            adjustment: total - base,
            total,
            deposit: total * 0.30,
            midpoint: total * 0.40,
            final_payment: total * 0.30,
            days: project_hours / hours_per_day,
        })
    } else {
        None
    };

    Ok(Rates {
        weeks_available,
        work_days,
        work_hours,
        billable_hours,
        billable_days,
        target_income,
        business_expenses,
        health_insurance,
        retirement,
        total_costs,
        pre_tax_need,
        income_tax,
        self_employment_tax,
        total_tax,
        required_revenue,
        effective_tax_percent,
        base_hourly,
        buffer_amount,
        hourly_rate,
        day_rate,
        week_rate,
        month_rate,
        worked_hour_rate,
        current_rate: current,
        quote,
    })
}

/// Map a complexity keyword to its display label and price multiplier.
fn complexity_multiplier(name: &str) -> Result<(String, f64), String> {
    match name.trim().to_ascii_lowercase().as_str() {
        "simple" => Ok(("Simple".into(), 1.0)),
        "standard" => Ok(("Standard".into(), 1.25)),
        "complex" => Ok(("Complex".into(), 1.5)),
        "rush" => Ok(("Rush".into(), 2.0)),
        other => Err(format!(
            "complexity must be one of simple, standard, complex, rush — got '{other}'"
        )),
    }
}

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

/// Format a plain number (hours, days) with thousands separators.
fn num_str(v: f64, dp: usize) -> String {
    money_str(v, "", dp)
}

/// Run the calculation and render it in the requested format.
#[allow(clippy::too_many_arguments)]
pub fn run(
    target_income: f64,
    business_expenses: f64,
    health_insurance: f64,
    retirement: f64,
    tax_rate: f64,
    tax_basis: &str,
    hours_per_week: f64,
    days_per_week: f64,
    hours_per_day: f64,
    weeks_per_year: f64,
    vacation_weeks: f64,
    holidays: f64,
    sick_days: f64,
    billable_percent: f64,
    buffer_percent: f64,
    current_rate: f64,
    project_hours: f64,
    complexity: &str,
    currency: &str,
    decimals: f64,
    format: &str,
) -> Result<String, String> {
    if !decimals.is_finite() || !(0.0..=6.0).contains(&decimals) {
        return Err("decimals must be a whole number between 0 and 6".into());
    }
    let dp = decimals.round() as usize;
    let currency = if currency.trim().is_empty() {
        ""
    } else {
        currency.trim()
    };
    if currency.chars().count() > 8 {
        return Err("currency must be at most 8 characters, for example '$', '€' or 'CHF '".into());
    }
    let r = compute(
        target_income,
        business_expenses,
        health_insurance,
        retirement,
        tax_rate,
        tax_basis,
        hours_per_week,
        days_per_week,
        hours_per_day,
        weeks_per_year,
        vacation_weeks,
        holidays,
        sick_days,
        billable_percent,
        buffer_percent,
        current_rate,
        project_hours,
        complexity,
    )?;

    match format.trim().to_ascii_lowercase().as_str() {
        "markdown" => Ok(render(&r, currency, dp, hours_per_day, true)),
        "text" => Ok(render(&r, currency, dp, hours_per_day, false)),
        "csv" => Ok(render_csv(&r, dp, hours_per_day)),
        "json" => Ok(render_json(&r, dp)),
        other => Err(format!(
            "format must be one of markdown, text, csv, json — got '{other}'"
        )),
    }
}

/// Rows shared by the markdown and plain-text renderers.
fn sections(
    r: &Rates,
    c: &str,
    dp: usize,
    hours_per_day: f64,
) -> Vec<(String, Vec<(String, String)>)> {
    let mut out: Vec<(String, Vec<(String, String)>)> = Vec::new();

    out.push((
        "Rate you need to charge".into(),
        vec![
            ("Hourly rate".into(), money_str(r.hourly_rate, c, dp)),
            (
                format!("Day rate ({}-hour day)", num_str(hours_per_day, 2)),
                money_str(r.day_rate, c, dp),
            ),
            (
                "Week rate (billable week)".into(),
                money_str(r.week_rate, c, dp),
            ),
            (
                "Month rate (average)".into(),
                money_str(r.month_rate, c, dp),
            ),
            (
                "Effective rate per worked hour".into(),
                money_str(r.worked_hour_rate, c, dp),
            ),
        ],
    ));

    out.push((
        "What the year has to earn".into(),
        vec![
            ("Take-home goal".into(), money_str(r.target_income, c, dp)),
            ("Income tax".into(), money_str(r.income_tax, c, dp)),
            (
                "Self-employment tax".into(),
                money_str(r.self_employment_tax, c, dp),
            ),
            (
                "Total tax".into(),
                format!(
                    "{} ({}% of profit)",
                    money_str(r.total_tax, c, dp),
                    num_str(r.effective_tax_percent, 1)
                ),
            ),
            (
                "Business expenses".into(),
                money_str(r.business_expenses, c, dp),
            ),
            (
                "Health insurance".into(),
                money_str(r.health_insurance, c, dp),
            ),
            ("Retirement".into(), money_str(r.retirement, c, dp)),
            (
                "Revenue you must invoice".into(),
                money_str(r.required_revenue, c, dp),
            ),
        ],
    ));

    out.push((
        "Time budget".into(),
        vec![
            (
                "Working weeks after time off".into(),
                num_str(r.weeks_available, 2),
            ),
            ("Working days".into(), num_str(r.work_days, 1)),
            ("Hours worked".into(), num_str(r.work_hours, 1)),
            ("Billable hours".into(), num_str(r.billable_hours, 1)),
            ("Billable days".into(), num_str(r.billable_days, 1)),
        ],
    ));

    if r.buffer_amount.abs() > 1e-12 {
        out.push((
            "Buffer".into(),
            vec![
                (
                    "Break-even hourly rate".into(),
                    money_str(r.base_hourly, c, dp),
                ),
                (
                    "Buffer added per hour".into(),
                    money_str(r.buffer_amount, c, dp),
                ),
                (
                    "Hourly rate charged".into(),
                    money_str(r.hourly_rate, c, dp),
                ),
            ],
        ));
    }

    if let Some(cur) = &r.current_rate {
        let verdict = if cur.rate_gap > 0.0 {
            format!("raise it by {}", money_str(cur.rate_gap, c, dp))
        } else if cur.rate_gap < 0.0 {
            format!("{} above target", money_str(-cur.rate_gap, c, dp))
        } else {
            "exactly on target".into()
        };
        out.push((
            "Your current rate".into(),
            vec![
                ("Current hourly rate".into(), money_str(cur.rate, c, dp)),
                ("Revenue at that rate".into(), money_str(cur.revenue, c, dp)),
                (
                    "Take-home at that rate".into(),
                    money_str(cur.take_home, c, dp),
                ),
                ("Versus your goal".into(), money_str(cur.income_gap, c, dp)),
                (
                    "Share of the required rate".into(),
                    format!("{}%", num_str(cur.percent_of_required, 1)),
                ),
                ("Verdict".into(), verdict),
            ],
        ));
    }

    if let Some(q) = &r.quote {
        out.push((
            "Project quote".into(),
            vec![
                (
                    "Estimated hours".into(),
                    format!("{} ({} days)", num_str(q.hours, 2), num_str(q.days, 2)),
                ),
                ("Base quote".into(), money_str(q.base, c, dp)),
                (
                    format!("{} adjustment (x{})", q.label, num_str(q.multiplier, 2)),
                    money_str(q.adjustment, c, dp),
                ),
                ("Total quote".into(), money_str(q.total, c, dp)),
                ("Deposit (30%)".into(), money_str(q.deposit, c, dp)),
                ("Midpoint (40%)".into(), money_str(q.midpoint, c, dp)),
                ("Final (30%)".into(), money_str(q.final_payment, c, dp)),
            ],
        ));
    }

    out
}

/// Markdown pipe tables, or the same content as aligned plain text.
fn render(r: &Rates, c: &str, dp: usize, hours_per_day: f64, markdown: bool) -> String {
    let secs = sections(r, c, dp, hours_per_day);
    let mut s = String::new();
    for (i, (title, rows)) in secs.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        if markdown {
            s.push_str(&format!("## {title}\n\n| Item | Value |\n| --- | --- |\n"));
            for (k, v) in rows {
                s.push_str(&format!("| {k} | {v} |\n"));
            }
        } else {
            s.push_str(&format!("{title}\n"));
            s.push_str(&format!("{}\n", "-".repeat(title.chars().count())));
            let width = rows
                .iter()
                .map(|(k, _)| k.chars().count())
                .max()
                .unwrap_or(0);
            for (k, v) in rows {
                s.push_str(&format!("{k:width$}  {v}\n"));
            }
        }
    }
    s.trim_end().to_string()
}

/// `section,item,value` rows, ready for a spreadsheet.
fn render_csv(r: &Rates, dp: usize, hours_per_day: f64) -> String {
    let secs = sections(r, "", dp, hours_per_day);
    let esc = |v: &str| {
        if v.contains(',') || v.contains('"') {
            format!("\"{}\"", v.replace('"', "\"\""))
        } else {
            v.to_string()
        }
    };
    let mut s = String::from("section,item,value\n");
    for (title, rows) in &secs {
        for (k, v) in rows {
            s.push_str(&format!("{},{},{}\n", esc(title), esc(k), esc(v)));
        }
    }
    s.trim_end().to_string()
}

/// The full result as JSON — every figure at the requested precision.
fn render_json(r: &Rates, dp: usize) -> String {
    let n = |v: f64| {
        let x = round_dp(v, dp);
        // Avoid "-0" in the output.
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
    let mut s = String::from("{\n");
    s.push_str(&format!("  \"hourly_rate\": {},\n", n(r.hourly_rate)));
    s.push_str(&format!("  \"day_rate\": {},\n", n(r.day_rate)));
    s.push_str(&format!("  \"week_rate\": {},\n", n(r.week_rate)));
    s.push_str(&format!("  \"month_rate\": {},\n", n(r.month_rate)));
    s.push_str(&format!(
        "  \"worked_hour_rate\": {},\n",
        n(r.worked_hour_rate)
    ));
    s.push_str(&format!("  \"break_even_hourly\": {},\n", n(r.base_hourly)));
    s.push_str(&format!("  \"buffer_per_hour\": {},\n", n(r.buffer_amount)));
    s.push_str(&format!(
        "  \"required_revenue\": {},\n",
        n(r.required_revenue)
    ));
    s.push_str(&format!("  \"target_income\": {},\n", n(r.target_income)));
    s.push_str(&format!("  \"income_tax\": {},\n", n(r.income_tax)));
    s.push_str(&format!(
        "  \"self_employment_tax\": {},\n",
        n(r.self_employment_tax)
    ));
    s.push_str(&format!("  \"total_tax\": {},\n", n(r.total_tax)));
    s.push_str(&format!(
        "  \"effective_tax_percent\": {},\n",
        n2(r.effective_tax_percent)
    ));
    s.push_str(&format!("  \"total_costs\": {},\n", n(r.total_costs)));
    s.push_str(&format!(
        "  \"business_expenses\": {},\n",
        n(r.business_expenses)
    ));
    s.push_str(&format!(
        "  \"health_insurance\": {},\n",
        n(r.health_insurance)
    ));
    s.push_str(&format!("  \"retirement\": {},\n", n(r.retirement)));
    s.push_str(&format!(
        "  \"weeks_available\": {},\n",
        n2(r.weeks_available)
    ));
    s.push_str(&format!("  \"work_days\": {},\n", n2(r.work_days)));
    s.push_str(&format!("  \"work_hours\": {},\n", n2(r.work_hours)));
    s.push_str(&format!(
        "  \"billable_hours\": {},\n",
        n2(r.billable_hours)
    ));
    s.push_str(&format!("  \"billable_days\": {}", n2(r.billable_days)));
    if let Some(c) = &r.current_rate {
        s.push_str(",\n  \"current_rate\": {\n");
        s.push_str(&format!("    \"rate\": {},\n", n(c.rate)));
        s.push_str(&format!("    \"revenue\": {},\n", n(c.revenue)));
        s.push_str(&format!("    \"take_home\": {},\n", n(c.take_home)));
        s.push_str(&format!("    \"rate_gap\": {},\n", n(c.rate_gap)));
        s.push_str(&format!("    \"income_gap\": {},\n", n(c.income_gap)));
        s.push_str(&format!(
            "    \"percent_of_required\": {}\n",
            n2(c.percent_of_required)
        ));
        s.push_str("  }");
    }
    if let Some(q) = &r.quote {
        s.push_str(",\n  \"quote\": {\n");
        s.push_str(&format!("    \"hours\": {},\n", n2(q.hours)));
        s.push_str(&format!("    \"days\": {},\n", n2(q.days)));
        s.push_str(&format!("    \"complexity\": \"{}\",\n", q.label));
        s.push_str(&format!("    \"multiplier\": {},\n", n2(q.multiplier)));
        s.push_str(&format!("    \"base\": {},\n", n(q.base)));
        s.push_str(&format!("    \"adjustment\": {},\n", n(q.adjustment)));
        s.push_str(&format!("    \"total\": {},\n", n(q.total)));
        s.push_str(&format!("    \"deposit\": {},\n", n(q.deposit)));
        s.push_str(&format!("    \"midpoint\": {},\n", n(q.midpoint)));
        s.push_str(&format!("    \"final\": {}\n", n(q.final_payment)));
        s.push_str("  }");
    }
    s.push_str("\n}");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Defaults matching the descriptor, so each test varies one thing.
    fn base() -> Rates {
        compute(
            80000.0,
            3000.0,
            0.0,
            0.0,
            30.0,
            "income_only",
            40.0,
            5.0,
            8.0,
            52.0,
            4.0,
            10.0,
            5.0,
            70.0,
            0.0,
            0.0,
            0.0,
            "standard",
        )
        .unwrap()
    }

    #[test]
    fn time_budget_subtracts_vacation_holidays_and_sick_days() {
        let r = base();
        // 52 weeks - 4 vacation - (10 + 5)/5 days-as-weeks = 45 weeks.
        assert_eq!(r.weeks_available, 45.0);
        assert_eq!(r.work_days, 225.0);
        assert_eq!(r.work_hours, 1800.0);
        assert_eq!(r.billable_hours, 1260.0);
    }

    #[test]
    fn tax_is_grossed_up_not_marked_up() {
        let r = base();
        // 80000 / (1 - 0.30) = 114285.714…, and 30% of that is the tax.
        assert!((r.income_tax + r.target_income - 114_285.714_285_71).abs() < 1e-6);
        assert!((r.required_revenue - (114_285.714_285_71 + 3000.0)).abs() < 1e-6);
        // Naive markup (80000 * 1.30 = 104000) would be far too low.
        assert!(r.required_revenue > 104_000.0 + 3000.0);
    }

    #[test]
    fn hourly_rate_is_required_revenue_over_billable_hours() {
        let r = base();
        assert!((r.hourly_rate - r.required_revenue / 1260.0).abs() < 1e-9);
        assert!((r.day_rate - r.hourly_rate * 8.0).abs() < 1e-9);
    }

    #[test]
    fn self_employment_layer_raises_the_rate() {
        let plain = base();
        let se = compute(
            80000.0,
            3000.0,
            0.0,
            0.0,
            30.0,
            "self_employment",
            40.0,
            5.0,
            8.0,
            52.0,
            4.0,
            10.0,
            5.0,
            70.0,
            0.0,
            0.0,
            0.0,
            "standard",
        )
        .unwrap();
        assert!(se.self_employment_tax > 0.0);
        assert!(se.hourly_rate > plain.hourly_rate);
        // The SE layer is 15.3% assessed on 92.35% of profit.
        let profit = se.target_income + se.total_tax;
        assert!((se.self_employment_tax - profit * 0.9235 * 0.153).abs() < 1e-6);
    }

    #[test]
    fn buffer_is_a_markup_on_the_rate() {
        let r = compute(
            80000.0,
            3000.0,
            0.0,
            0.0,
            30.0,
            "income_only",
            40.0,
            5.0,
            8.0,
            52.0,
            4.0,
            10.0,
            5.0,
            70.0,
            10.0,
            0.0,
            0.0,
            "standard",
        )
        .unwrap();
        assert!((r.hourly_rate - r.base_hourly * 1.1).abs() < 1e-9);
        assert!((r.buffer_amount - r.base_hourly * 0.1).abs() < 1e-9);
    }

    #[test]
    fn current_rate_reports_the_gap_in_both_directions() {
        let r = base();
        let low = compute(
            80000.0,
            3000.0,
            0.0,
            0.0,
            30.0,
            "income_only",
            40.0,
            5.0,
            8.0,
            52.0,
            4.0,
            10.0,
            5.0,
            70.0,
            0.0,
            50.0,
            0.0,
            "standard",
        )
        .unwrap();
        let c = low.current_rate.unwrap();
        assert_eq!(c.rate, 50.0);
        assert!(
            c.rate_gap > 0.0,
            "50/h is under the required {}",
            r.hourly_rate
        );
        assert!(c.income_gap < 0.0);
        // 50/h over 1260 billable hours, minus 3000 costs, times the 70% kept.
        assert!((c.revenue - 63_000.0).abs() < 1e-9);
        assert!((c.take_home - 60_000.0 * 0.7).abs() < 1e-6);
    }

    #[test]
    fn project_quote_applies_the_complexity_multiplier_and_milestones() {
        let r = compute(
            80000.0,
            3000.0,
            0.0,
            0.0,
            30.0,
            "income_only",
            40.0,
            5.0,
            8.0,
            52.0,
            4.0,
            10.0,
            5.0,
            70.0,
            0.0,
            0.0,
            40.0,
            "rush",
        )
        .unwrap();
        let q = r.quote.unwrap();
        assert_eq!(q.multiplier, 2.0);
        assert!((q.base - r.hourly_rate * 40.0).abs() < 1e-9);
        assert!((q.total - q.base * 2.0).abs() < 1e-9);
        assert!((q.deposit + q.midpoint + q.final_payment - q.total).abs() < 1e-9);
        assert_eq!(q.days, 5.0);
    }

    #[test]
    fn no_quote_and_no_current_rate_when_left_at_zero() {
        let r = base();
        assert!(r.quote.is_none());
        assert!(r.current_rate.is_none());
    }

    #[test]
    fn money_is_grouped_and_prefixed() {
        assert_eq!(money_str(1234567.891, "$", 2), "$1,234,567.89");
        assert_eq!(money_str(-42.5, "€", 0), "-€43");
        assert_eq!(money_str(999.0, "", 2), "999.00");
    }

    #[test]
    fn markdown_reports_the_headline_rate() {
        let out = run(
            80000.0,
            3000.0,
            0.0,
            0.0,
            30.0,
            "income_only",
            40.0,
            5.0,
            8.0,
            52.0,
            4.0,
            10.0,
            5.0,
            70.0,
            0.0,
            0.0,
            0.0,
            "standard",
            "$",
            2.0,
            "markdown",
        )
        .unwrap();
        assert!(out.starts_with("## Rate you need to charge"));
        assert!(out.contains("| Hourly rate | $93.08 |"), "got:\n{out}");
        assert!(
            out.contains("| Day rate (8.00-hour day) | $744.67 |"),
            "got:\n{out}"
        );
    }

    #[test]
    fn csv_and_json_formats_render() {
        let args = |f: &str| {
            run(
                80000.0,
                3000.0,
                0.0,
                0.0,
                30.0,
                "income_only",
                40.0,
                5.0,
                8.0,
                52.0,
                4.0,
                10.0,
                5.0,
                70.0,
                0.0,
                0.0,
                0.0,
                "standard",
                "$",
                2.0,
                f,
            )
            .unwrap()
        };
        let csv = args("csv");
        assert!(csv.starts_with("section,item,value\n"));
        assert!(csv.contains("Rate you need to charge,Hourly rate,93.08"));
        let json = args("json");
        assert!(json.contains("\"hourly_rate\": 93.08"), "got:\n{json}");
        assert!(json.contains("\"billable_hours\": 1260"), "got:\n{json}");
    }

    #[test]
    fn text_format_is_aligned_not_piped() {
        let out = run(
            80000.0,
            3000.0,
            0.0,
            0.0,
            30.0,
            "income_only",
            40.0,
            5.0,
            8.0,
            52.0,
            4.0,
            10.0,
            5.0,
            70.0,
            0.0,
            0.0,
            0.0,
            "standard",
            "$",
            0.0,
            "text",
        )
        .unwrap();
        assert!(out.starts_with("Rate you need to charge\n-----"));
        assert!(!out.contains('|'));
        assert!(out.contains("$93"));
    }

    #[test]
    fn rejects_a_zero_income_goal() {
        let err = compute(
            0.0,
            0.0,
            0.0,
            0.0,
            30.0,
            "income_only",
            40.0,
            5.0,
            8.0,
            52.0,
            4.0,
            10.0,
            5.0,
            70.0,
            0.0,
            0.0,
            0.0,
            "standard",
        )
        .unwrap_err();
        assert!(err.contains("target_income"), "got: {err}");
    }

    #[test]
    fn rejects_a_year_with_no_working_weeks_left() {
        let err = compute(
            80000.0,
            0.0,
            0.0,
            0.0,
            30.0,
            "income_only",
            40.0,
            5.0,
            8.0,
            52.0,
            52.0,
            0.0,
            0.0,
            70.0,
            0.0,
            0.0,
            0.0,
            "standard",
        )
        .unwrap_err();
        assert!(err.contains("no working weeks left"), "got: {err}");
    }

    #[test]
    fn rejects_an_impossible_tax_rate_combination() {
        let err = compute(
            80000.0,
            0.0,
            0.0,
            0.0,
            90.0,
            "self_employment",
            40.0,
            5.0,
            8.0,
            52.0,
            4.0,
            10.0,
            5.0,
            70.0,
            0.0,
            0.0,
            0.0,
            "standard",
        )
        .unwrap_err();
        assert!(err.contains("nothing to take home"), "got: {err}");
    }

    #[test]
    fn rejects_bad_enums_and_out_of_range_numbers() {
        assert!(compute(
            80000.0, 0.0, 0.0, 0.0, 30.0, "nope", 40.0, 5.0, 8.0, 52.0, 4.0, 10.0, 5.0, 70.0, 0.0,
            0.0, 0.0, "standard"
        )
        .unwrap_err()
        .contains("tax_basis"));
        assert!(compute(
            80000.0,
            0.0,
            0.0,
            0.0,
            30.0,
            "income_only",
            40.0,
            5.0,
            8.0,
            52.0,
            4.0,
            10.0,
            5.0,
            0.0,
            0.0,
            0.0,
            0.0,
            "standard"
        )
        .unwrap_err()
        .contains("billable_percent"));
        assert!(compute(
            80000.0,
            0.0,
            0.0,
            0.0,
            30.0,
            "income_only",
            40.0,
            5.0,
            8.0,
            52.0,
            4.0,
            10.0,
            5.0,
            70.0,
            0.0,
            0.0,
            10.0,
            "wat"
        )
        .unwrap_err()
        .contains("complexity"));
        assert!(run(
            80000.0,
            0.0,
            0.0,
            0.0,
            30.0,
            "income_only",
            40.0,
            5.0,
            8.0,
            52.0,
            4.0,
            10.0,
            5.0,
            70.0,
            0.0,
            0.0,
            0.0,
            "standard",
            "$",
            2.0,
            "xml"
        )
        .unwrap_err()
        .contains("format"));
    }

    #[test]
    fn a_four_day_week_loses_the_right_share_of_the_year() {
        let r = compute(
            80000.0,
            0.0,
            0.0,
            0.0,
            30.0,
            "income_only",
            32.0,
            4.0,
            8.0,
            52.0,
            4.0,
            8.0,
            0.0,
            70.0,
            0.0,
            0.0,
            0.0,
            "standard",
        )
        .unwrap();
        // 8 holidays at 4 days/week = 2 weeks, so 52 - 4 - 2 = 46 weeks.
        assert_eq!(r.weeks_available, 46.0);
        assert_eq!(r.work_days, 184.0);
        assert_eq!(r.work_hours, 1472.0);
    }
}
