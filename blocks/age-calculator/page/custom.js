// age-calculator page module — renders the core's JSON result as the age
// dashboard (hero + info cards + lifetime metrics). Inputs/defaults/reset are
// fully declarative in meta.toml (kind="date", default="today", [[example]]);
// only the bespoke result layout lives here. Loaded by the shared tool.js via
// the generator's page/custom.js hook; styles in custom.css.

export function renderResult(value, ctx) {
  let data;
  try {
    data = JSON.parse(value);
  } catch (e) {
    return false; // not the dashboard payload — let the generic text path render
  }
  const out = ctx.out;
  out.innerHTML = "";
  out.className = "age-active-container";

  // 1. Hero Summary
  const hero = document.createElement("div");
  hero.className = "age-result-hero";

  const heroTitle = document.createElement("div");
  heroTitle.className = "age-result-hero-title";
  heroTitle.textContent = "Exact Age";
  hero.appendChild(heroTitle);

  const heroText = document.createElement("div");
  heroText.className = "age-result-hero-text";

  const yearsSpan = `<span>${data.years}</span> year${data.years === 1 ? '' : 's'}`;
  const monthsSpan = `<span>${data.months}</span> month${data.months === 1 ? '' : 's'}`;
  const daysSpan = `<span>${data.days}</span> day${data.days === 1 ? '' : 's'}`;

  heroText.innerHTML = `${yearsSpan}, ${monthsSpan}, and ${daysSpan} old`;
  hero.appendChild(heroText);
  out.appendChild(hero);

  // 2. Info Cards Grid (Next Birthday, Birth Details, Zodiac)
  const infoGrid = document.createElement("div");
  infoGrid.className = "age-info-grid";

  const bdayCard = document.createElement("div");
  bdayCard.className = "age-info-card";

  const bdayTitle = document.createElement("div");
  bdayTitle.className = "age-info-card-title";
  bdayTitle.textContent = "Next Birthday";
  bdayCard.appendChild(bdayTitle);

  const bdayValue = document.createElement("div");
  bdayValue.className = "age-info-card-value";
  if (data.days_until_next_birthday === 0) {
    bdayValue.textContent = "Today! 🎉";
  } else {
    bdayValue.textContent = `${data.days_until_next_birthday} Day${data.days_until_next_birthday === 1 ? '' : 's'}`;
  }
  bdayCard.appendChild(bdayValue);

  const bdaySub = document.createElement("div");
  bdaySub.className = "age-info-card-sub";
  bdaySub.textContent = `Date: ${data.next_birthday}`;
  bdayCard.appendChild(bdaySub);

  infoGrid.appendChild(bdayCard);

  const detailsCard = document.createElement("div");
  detailsCard.className = "age-info-card";

  const detailsTitle = document.createElement("div");
  detailsTitle.className = "age-info-card-title";
  detailsTitle.textContent = "Born On";
  detailsCard.appendChild(detailsTitle);

  const detailsValue = document.createElement("div");
  detailsValue.className = "age-info-card-value";
  detailsValue.textContent = data.day_of_week_born;
  detailsCard.appendChild(detailsValue);

  const detailsSub = document.createElement("div");
  detailsSub.className = "age-info-card-sub";
  detailsSub.textContent = `Birthdate: ${data.birthdate}`;
  detailsCard.appendChild(detailsSub);

  infoGrid.appendChild(detailsCard);

  const zodiacCard = document.createElement("div");
  zodiacCard.className = "age-info-card";

  const zodiacTitle = document.createElement("div");
  zodiacTitle.className = "age-info-card-title";
  zodiacTitle.textContent = "Zodiac & Generation";
  zodiacCard.appendChild(zodiacTitle);

  const zodiacValue = document.createElement("div");
  zodiacValue.className = "age-info-card-value";
  zodiacValue.textContent = `${data.zodiac_sign} / ${data.chinese_zodiac}`;
  zodiacCard.appendChild(zodiacValue);

  const zodiacSub = document.createElement("div");
  zodiacSub.className = "age-info-card-sub";
  zodiacSub.textContent = data.generation;
  zodiacCard.appendChild(zodiacSub);

  infoGrid.appendChild(zodiacCard);
  out.appendChild(infoGrid);

  // 3. Lifetime Metrics
  const livedCard = document.createElement("div");
  livedCard.className = "age-lived-card";

  const livedTitle = document.createElement("div");
  livedTitle.className = "age-lived-card-title";
  livedTitle.textContent = "Lifetime Metrics";
  livedCard.appendChild(livedTitle);

  const livedGrid = document.createElement("div");
  livedGrid.className = "age-lived-grid";

  const addMetric = (val, label) => {
    const item = document.createElement("div");
    item.className = "age-lived-item";
    const valueEl = document.createElement("div");
    valueEl.className = "age-lived-item-value";
    valueEl.textContent = val.toLocaleString();
    const labelEl = document.createElement("div");
    labelEl.className = "age-lived-item-label";
    labelEl.textContent = label;
    item.appendChild(valueEl);
    item.appendChild(labelEl);
    livedGrid.appendChild(item);
  };

  addMetric(data.total_months, "Total Months");
  addMetric(data.total_weeks, "Total Weeks");
  addMetric(data.total_days, "Total Days");
  addMetric(data.total_hours, "Total Hours");
  addMetric(data.total_minutes, "Total Minutes");
  addMetric(data.total_seconds, "Total Seconds");

  livedCard.appendChild(livedGrid);
  out.appendChild(livedCard);
  return true;
}
