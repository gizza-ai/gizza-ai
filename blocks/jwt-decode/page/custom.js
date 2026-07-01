// jwt-decode page module — renders the core's JSON result as the decode
// dashboard (colorized encoded token + claims validation + decoded header/
// payload cards). Inputs stay the shared declarative controls; only the
// bespoke result layout lives here. Loaded by the shared tool.js via the
// generator's page/custom.js hook; styles in custom.css.

export function setup(ctx) {
  // Dashboard output — the generic Copy-result button would copy the cards'
  // concatenated labels as text soup, so drop it. Reset stays (the token
  // field is a normal input).
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
  out.className = "jwt-decode-container";

  const grid = document.createElement("div");
  grid.className = "jwt-grid";

  // Left Column: Encoded Token Colorized View
  const leftCol = document.createElement("div");
  leftCol.className = "jwt-col-left";

  const visualTitle = document.createElement("h3");
  visualTitle.className = "jwt-section-title";
  visualTitle.textContent = "Encoded Token";
  leftCol.appendChild(visualTitle);

  const highlightBox = document.createElement("div");
  highlightBox.className = "jwt-highlight-box";

  const tokenInput = document.getElementById("in-token");
  const tokenStr = tokenInput ? tokenInput.value.trim() : "";
  const parts = tokenStr.split(".");

  if (parts.length >= 1 && parts[0]) {
    const spanHeader = document.createElement("span");
    spanHeader.className = "jwt-token-part jwt-color-header";
    spanHeader.textContent = parts[0];
    highlightBox.appendChild(spanHeader);
  }
  if (parts.length >= 2) {
    const dot1 = document.createTextNode(".");
    highlightBox.appendChild(dot1);
    const spanPayload = document.createElement("span");
    spanPayload.className = "jwt-token-part jwt-color-payload";
    spanPayload.textContent = parts[1];
    highlightBox.appendChild(spanPayload);
  }
  if (parts.length >= 3) {
    const dot2 = document.createTextNode(".");
    highlightBox.appendChild(dot2);
    const spanSignature = document.createElement("span");
    spanSignature.className = "jwt-token-part jwt-color-signature";
    spanSignature.textContent = parts[2];
    highlightBox.appendChild(spanSignature);
  }
  leftCol.appendChild(highlightBox);
  grid.appendChild(leftCol);

  // Right Column: Claims Validation & Decoded JSON
  const rightCol = document.createElement("div");
  rightCol.className = "jwt-col-right";

  const statusBanner = document.createElement("div");
  statusBanner.className = `jwt-status-banner ${data.valid ? "jwt-status-valid" : "jwt-status-invalid"}`;

  const statusText = document.createElement("div");
  statusText.className = "jwt-status-text";
  statusText.innerHTML = data.valid
    ? `<strong>Active Token</strong> — Standard time claims are currently valid.`
    : `<strong>Validation Flagged</strong> — ${data.error || "Token has expired or is not yet valid."}`;
  statusBanner.appendChild(statusText);
  rightCol.appendChild(statusBanner);

  // Claims Validation Card
  const checksCard = document.createElement("div");
  checksCard.className = "jwt-card jwt-checks-card";

  const checksTitle = document.createElement("h4");
  checksTitle.className = "jwt-card-title";
  checksTitle.textContent = "Claims Validation";
  checksCard.appendChild(checksTitle);

  const checksList = document.createElement("div");
  checksList.className = "jwt-checks-list";

  data.checks.forEach((c) => {
    const checkItem = document.createElement("div");
    checkItem.className = `jwt-check-item ${c.ok ? "jwt-check-ok" : "jwt-check-fail"}`;

    const icon = document.createElement("span");
    icon.className = "jwt-check-icon";
    icon.innerHTML = c.ok
      ? `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>`
      : `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12.01" y2="16"></line></svg>`;
    checkItem.appendChild(icon);

    const info = document.createElement("div");
    info.className = "jwt-check-info";

    const name = document.createElement("div");
    name.className = "jwt-check-name";
    name.textContent = c.name === "signature_present" ? "Signature segment" : c.name.toUpperCase();
    info.appendChild(name);

    const desc = document.createElement("div");
    desc.className = "jwt-check-desc";
    desc.textContent = c.detail;
    info.appendChild(desc);

    checkItem.appendChild(info);
    checksList.appendChild(checkItem);
  });
  checksCard.appendChild(checksList);
  rightCol.appendChild(checksCard);

  // Decoded Header Card
  const headerCard = document.createElement("div");
  headerCard.className = "jwt-card jwt-decoded-card jwt-decoded-header-card";

  const headerTitleWrap = document.createElement("div");
  headerTitleWrap.className = "jwt-card-title-wrap";

  const headerTitle = document.createElement("h4");
  headerTitle.className = "jwt-card-title jwt-color-header-text";
  headerTitle.textContent = "HEADER: ALGORITHM & TOKEN TYPE";
  headerTitleWrap.appendChild(headerTitle);

  const headerCopy = document.createElement("button");
  headerCopy.className = "jwt-copy-btn";
  headerCopy.title = "Copy Header JSON";
  const headerJsonStr = JSON.stringify(data.header, null, 2);
  headerCopy.addEventListener("click", () => {
    navigator.clipboard.writeText(headerJsonStr).then(() => {
      headerCopy.classList.add("copied");
      setTimeout(() => headerCopy.classList.remove("copied"), 2000);
    });
  });
  headerCopy.innerHTML = `<svg class="copy-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path></svg><svg class="check-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>`;
  headerTitleWrap.appendChild(headerCopy);
  headerCard.appendChild(headerTitleWrap);

  const headerCode = document.createElement("pre");
  headerCode.className = "jwt-code-block";
  headerCode.textContent = headerJsonStr;
  headerCard.appendChild(headerCode);
  rightCol.appendChild(headerCard);

  // Decoded Payload Card
  const payloadCard = document.createElement("div");
  payloadCard.className = "jwt-card jwt-decoded-card jwt-decoded-payload-card";

  const payloadTitleWrap = document.createElement("div");
  payloadTitleWrap.className = "jwt-card-title-wrap";

  const payloadTitle = document.createElement("h4");
  payloadTitle.className = "jwt-card-title jwt-color-payload-text";
  payloadTitle.textContent = "PAYLOAD: DATA / CLAIMS";
  payloadTitleWrap.appendChild(payloadTitle);

  const payloadCopy = document.createElement("button");
  payloadCopy.className = "jwt-copy-btn";
  payloadCopy.title = "Copy Payload JSON";
  const payloadJsonStr = JSON.stringify(data.payload, null, 2);
  payloadCopy.addEventListener("click", () => {
    navigator.clipboard.writeText(payloadJsonStr).then(() => {
      payloadCopy.classList.add("copied");
      setTimeout(() => payloadCopy.classList.remove("copied"), 2000);
    });
  });
  payloadCopy.innerHTML = `<svg class="copy-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path></svg><svg class="check-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>`;
  payloadTitleWrap.appendChild(payloadCopy);
  payloadCard.appendChild(payloadTitleWrap);

  const payloadCode = document.createElement("pre");
  payloadCode.className = "jwt-code-block";
  payloadCode.textContent = payloadJsonStr;
  payloadCard.appendChild(payloadCode);
  rightCol.appendChild(payloadCard);

  grid.appendChild(rightCol);
  out.appendChild(grid);
  return true;
}
