// timezone-convert page module — renders the core's JSON result as the
// timezone dashboard (target cards + 24-hour meeting-planner grid). Inputs are
// fully declarative in meta.toml (datetime-local picker, timezone datalist,
// tag-list targets, defaults, wide); only the bespoke result layout lives
// here. Loaded by the shared tool.js via the generator's page/custom.js hook;
// styles in custom.css.

export function setup(ctx) {
  // Dashboard output — the generic Copy-result button would copy the cards +
  // planner grid as text soup, so drop it. Everything else (defaults, tag
  // list, Reset, deep-links, recompute wiring) is the shared driver's.
  document.getElementById("tool-copy-output")?.remove();
}

export function renderResult(value, ctx) {
  let data;
  try {
    data = JSON.parse(value);
  } catch (e) {
    return false; // not the dashboard payload — let the generic text path render
  }
  const out = ctx.out;
  out.innerHTML = "";
  out.className = "tz-active-container";

  const grid = document.createElement("div");
  grid.className = "tz-target-grid";

  data.targets.forEach((t) => {
    const card = document.createElement("div");
    card.className = "tz-card";

    const zone = document.createElement("div");
    zone.className = "tz-card-zone";
    zone.textContent = t.to_zone;
    card.appendChild(zone);

    const time = document.createElement("div");
    time.className = "tz-card-time";
    const prettyParts = t.to_pretty.split(" ");
    const timeStr = prettyParts[prettyParts.length - 1];
    time.textContent = timeStr;
    card.appendChild(time);

    const date = document.createElement("div");
    date.className = "tz-card-date";
    date.textContent = prettyParts.slice(0, prettyParts.length - 1).join(" ");
    card.appendChild(date);

    const meta = document.createElement("div");
    meta.className = "tz-card-meta";

    const badgeOffset = document.createElement("span");
    badgeOffset.className = "tz-badge tz-badge-offset";
    badgeOffset.textContent = t.to_offset;
    meta.appendChild(badgeOffset);

    if (t.to_is_dst) {
      const badgeDst = document.createElement("span");
      badgeDst.className = "tz-badge tz-badge-dst";
      badgeDst.textContent = "DST";
      meta.appendChild(badgeDst);
    }

    const diffHours = t.offset_diff_hours;
    const badgeDiff = document.createElement("span");
    badgeDiff.className = "tz-badge tz-badge-diff";
    if (diffHours === 0) {
      badgeDiff.textContent = "Same time";
    } else {
      const sign = diffHours > 0 ? "+" : "";
      badgeDiff.textContent = `${sign}${diffHours}h`;
    }
    meta.appendChild(badgeDiff);
    card.appendChild(meta);

    grid.appendChild(card);
  });
  out.appendChild(grid);

  const planner = document.createElement("div");
  planner.className = "tz-planner-section";

  const title = document.createElement("div");
  title.className = "tz-planner-title";
  title.textContent = `Meeting Planner Grid (Source: ${data.from_zone})`;
  planner.appendChild(title);

  const wrapper = document.createElement("div");
  wrapper.className = "tz-planner-table-wrapper";

  const table = document.createElement("table");
  table.className = "tz-planner-table";

  const thead = document.createElement("thead");
  const headerRow = document.createElement("tr");

  const thSource = document.createElement("th");
  thSource.textContent = `${data.from_zone} (Source)`;
  headerRow.appendChild(thSource);

  data.targets.forEach((t) => {
    const thTarget = document.createElement("th");
    thTarget.textContent = t.to_zone;
    headerRow.appendChild(thTarget);
  });
  thead.appendChild(headerRow);
  table.appendChild(thead);

  const tbody = document.createElement("tbody");
  data.meeting_planner.forEach((slot) => {
    const row = document.createElement("tr");
    row.className = "tz-planner-row";

    const inputHour = parseInt(data.from.split("T")[1].split(":")[0], 10);
    if (slot.from_hour === inputHour) {
      row.classList.add("selected");
    }

    const tdSource = document.createElement("td");
    const spanSourceTime = document.createElement("span");
    spanSourceTime.style.fontWeight = "bold";
    spanSourceTime.style.marginRight = "8px";
    spanSourceTime.textContent = slot.from_time;
    tdSource.appendChild(spanSourceTime);

    const badgeSource = document.createElement("span");
    badgeSource.className = `tz-status-badge tz-status-${slot.from_status.toLowerCase()}`;
    badgeSource.textContent = slot.from_status;
    tdSource.appendChild(badgeSource);
    row.appendChild(tdSource);

    slot.targets.forEach((t) => {
      const tdTarget = document.createElement("td");
      const spanTargetTime = document.createElement("span");
      spanTargetTime.style.fontWeight = "bold";
      spanTargetTime.style.marginRight = "8px";
      spanTargetTime.textContent = t.to_time;
      tdTarget.appendChild(spanTargetTime);

      const badgeTarget = document.createElement("span");
      badgeTarget.className = `tz-status-badge tz-status-${t.to_status.toLowerCase()}`;
      badgeTarget.textContent = t.to_status;
      tdTarget.appendChild(badgeTarget);
      row.appendChild(tdTarget);
    });

    row.addEventListener("click", () => {
      tbody.querySelectorAll(".tz-planner-row").forEach((r) => r.classList.remove("selected"));
      row.classList.add("selected");
    });

    tbody.appendChild(row);
  });
  table.appendChild(tbody);
  wrapper.appendChild(table);
  planner.appendChild(wrapper);
  out.appendChild(planner);
  return true;
}
