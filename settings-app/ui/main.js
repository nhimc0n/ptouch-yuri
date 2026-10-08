// SPDX-License-Identifier: GPL-3.0-or-later
//
// UI only. It never builds printer bytes: it reads and writes the driver
// settings and shows the printer's state through the Tauri commands in
// src-tauri/src/main.rs. All text comes from i18n.js (Vietnamese or English).
// Outside Tauri (a plain browser) it runs on made-up data so every state can be
// looked at: ?m=ok|loi|dang-in|mat-ket-noi|chua-cai|tai   (&lang=en to force English)

const tauri = window.__TAURI__?.core;
const params = new URLSearchParams(location.search);
const mode = params.get("m") || "ok";
if (params.get("lang") === "en" || params.get("lang") === "vi") lang = params.get("lang");

// ---------------------------------------------------------------- icons
const ICONS = {
  printer: '<path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/><path d="M6 9V3a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v6"/><rect x="6" y="14" width="12" height="8" rx="1"/>',
  refresh: '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>',
  lock: '<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>',
  check: '<path d="M20 6 9 17l-5-5"/>',
  chevron: '<path d="m6 9 6 6 6-6"/>',
  warn: '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>',
  alert: '<circle cx="12" cy="12" r="10"/><line x1="12" x2="12" y1="8" y2="12"/><line x1="12" x2="12.01" y1="16" y2="16"/>',
};
// Replace an element's markup only when it changed, so screen readers do not
// re-announce the same content and a focused control inside is not destroyed
// for nothing. When it is replaced, keyboard focus goes back to the same id.
const painted = new WeakMap();
function setMarkup(el, html) {
  if (painted.get(el) === html) return false;
  const focusId = el.contains(document.activeElement) ? document.activeElement.id : "";
  el.innerHTML = html;
  painted.set(el, html);
  if (focusId) document.getElementById(focusId)?.focus();
  return true;
}
const icon = (name) => `<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name]}</svg>`;
const paintStaticIcons = () => {
  for (const el of document.querySelectorAll("svg.icon[data-i]")) {
    el.setAttribute("viewBox", "0 0 24 24");
    el.setAttribute("aria-hidden", "true");
    el.innerHTML = ICONS[el.dataset.i];
  }
};

// ---------------------------------------------------------------- backend
const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const MOCK = {
  settings: {
    half_cut: true, full_cut: false, chain: false, mirror: false, quality: "Normal", page_size: "Auto",
    page_sizes: ["Auto", "L50", "L75", "L100", "L150", "L200", "L300", "P50", "P75", "P100", "P150", "P200", "AutoP", "Auto9", "S9L50", "S9L100", "S9L200"],
  },
  printers: {
    ok: { host: "192.168.99.107", status: { model: "PT-E850TKW", ready: true, state: "idle", tape_mm: 36, tape_supported: true }, error: null },
    loi: { host: "192.168.99.107", status: { model: "PT-E850TKW", ready: false, state: "error", tape_mm: 9, tape_supported: true }, error: null },
    "dang-in": { host: "192.168.99.107", status: { model: "PT-E850TKW", ready: false, state: "printing", tape_mm: 36, tape_supported: true }, error: null },
    "mat-ket-noi": { host: "192.168.99.107", status: null, error: "Cannot reach the printer at 192.168.99.107: SNMP receive timed out" },
  },
};
async function call(command, args) {
  if (tauri) return tauri.invoke(command, args);
  await wait(mode === "tai" ? 60000 : 350);
  if (command === "get_settings") {
    if (mode === "chua-cai") throw "The print queue PT-E850TKW was not found. Install the driver first.";
    return structuredClone(MOCK.settings);
  }
  if (command === "test_print") {
    if (mode === "loi" || mode === "dang-in") throw "The printer is not ready (state: " + (mode === "loi" ? "error" : "printing") + "). Check it and try again.";
    return "PT-E850TKW-1";
  }
  if (command === "set_printer_host" && !/^[\w.-]+$/.test(args.host.trim())) throw "Enter the printer's IP address or host name, for example 192.168.99.107.";
  if (command === "get_printer") return structuredClone(MOCK.printers[mode] || MOCK.printers.ok);
  return null;
}

// ---------------------------------------------------------------- sizes
function sizeInfo(key) {
  const fixed = key.match(/^L(\d+)$/);
  const portrait = key.match(/^P(\d+)$/);
  const nine = key.match(/^S9L(\d+)$/);
  if (key === "Auto" || key === "AutoP") return { group: "size.g.auto", label: t(`size.${key}`), extra: t("size.tape", { mm: 36 }), tape: 36 };
  if (key === "Auto9") return { group: "size.g.auto", label: t("size.Auto9"), extra: t("size.tape", { mm: 9 }), tape: 9 };
  if (fixed) return { group: "size.g.l36", label: `${fixed[1]} × 36 mm`, extra: "", tape: 36 };
  if (portrait) return { group: "size.g.p36", label: `36 × ${portrait[1]} mm`, extra: "", tape: 36 };
  if (nine) return { group: "size.g.s9", label: `${nine[1]} × 9 mm`, extra: "", tape: 9 };
  return { group: "size.g.other", label: key, extra: "", tape: 0 };
}
const sizeText = (key) => { const s = sizeInfo(key); return s.extra ? `${s.label} · ${s.extra}` : s.label; };

const QUALITY_KEYS = ["Normal", "High", "HiRes"];
const DEFAULTS = { half_cut: true, full_cut: false, chain: false, mirror: false, quality: "Normal", page_size: "Auto" };

// ---------------------------------------------------------------- state
const $ = (id) => document.getElementById(id);
const state = { saved: null, form: null, printer: null, loadingPrinter: true, queueError: false, applied: false, applying: false, testing: false, sent: false, error: null, hostError: null };
const keys = ["half_cut", "full_cut", "chain", "mirror", "quality", "page_size"];
const isDirty = () => !!state.saved && keys.some((k) => state.form[k] !== state.saved[k]);

// ---------------------------------------------------------------- render
function renderStatus() {
  const box = $("status");
  const info = state.printer;
  if (state.loadingPrinter && !info) {
    box.dataset.tone = "none";
    setMarkup(box, `<div class="skeleton" style="width:48px;height:48px;border-radius:12px"></div><div class="status-text"><div class="skeleton" style="width:160px;height:20px"></div><div class="skeleton" style="width:240px;height:16px;margin-top:8px"></div></div>`);
    return;
  }
  const st = info?.status;
  let tone = "none";
  let title = t("st.noconn");
  let sub = info?.host ? t("st.address", { host: info.host }) : t("st.noaddress");
  if (st) {
    sub = `${st.model} · ${info.host}`;
    const map = { idle: ["ok", "st.idle"], printing: ["busy", "st.printing"], error: ["bad", "st.error"], warmup: ["warn", "st.warmup"] };
    const [tn, key] = map[st.state] || ["warn", "st.unknown"];
    tone = tn; title = t(key);
  } else if (info?.error) tone = "bad";
  box.dataset.tone = tone;
  const want = state.form ? sizeInfo(state.form.page_size).tape : 0;
  const tapeOk = st && st.tape_supported && (!want || want === st.tape_mm);
  const tape = st ? `<span class="badge" data-tone="${tapeOk ? "ok" : "warn"}">${t("st.tape", { mm: st.tape_mm })}</span>` : "";
  const changed = setMarkup(box, `<div class="status-icon">${icon("printer")}</div>
    <div class="status-text"><div class="status-title">${title}</div><div class="status-sub">${sub}</div></div>
    <div class="status-side">${tape}<button class="icon-btn" id="refresh" type="button" aria-label="${t("st.refresh")}" title="${t("st.refresh")}" ${state.loadingPrinter ? 'aria-busy="true"' : ""}>${icon("refresh")}</button></div>`);
  if (changed) $("refresh").addEventListener("click", loadPrinter);
}

function banner(tone, iconName, title, desc, action) {
  return `<div class="banner" data-tone="${tone}" role="${tone === "bad" ? "alert" : "status"}">${icon(iconName)}
    <div class="banner-body"><p class="banner-title">${title}</p><p class="banner-desc">${desc}</p></div>
    ${action ? `<div class="banner-act"><button class="btn btn-outline" type="button" data-act="${action.id}">${action.label}</button></div>` : ""}</div>`;
}

function renderAlerts() {
  const out = [];
  const info = state.printer;
  const st = info?.status;
  if (state.queueError) {
    out.push(banner("bad", "alert", t("b.noqueue.t"), t("b.noqueue.d")));
  } else if (!state.loadingPrinter && !st) {
    out.push(banner("bad", "alert", t("b.noconn.t"), t("b.noconn.d", { detail: info?.error || t("b.noconn.none") }), { id: "refresh", label: t("b.retry") }));
  } else if (st?.state === "error") {
    out.push(banner("bad", "alert", t("b.error.t"), t("b.error.d"), { id: "refresh", label: t("b.refresh") }));
  }
  if (st && !st.tape_supported) {
    out.push(banner("warn", "warn", t("b.unsupported.t", { mm: st.tape_mm }), t("b.unsupported.d")));
  } else if (st && state.form) {
    const want = sizeInfo(state.form.page_size).tape;
    if (want && want !== st.tape_mm) {
      const suggest = st.tape_mm === 9 ? "Auto9" : "Auto";
      out.push(banner("warn", "warn", t("b.mismatch.t", { have: st.tape_mm, want }), t("b.mismatch.d"),
        state.form.page_sizes?.includes(suggest) ? { id: "fix-size", label: t("b.mismatch.a", { have: st.tape_mm }) } : null));
    }
  }
  if (!setMarkup($("alerts"), out.join(""))) return;
  for (const button of document.querySelectorAll("[data-act]")) {
    button.addEventListener("click", () => {
      if (button.dataset.act === "refresh") { loadPrinter(); $("refresh")?.focus(); }
      if (button.dataset.act === "fix-size") { state.form.page_size = state.printer.status.tape_mm === 9 ? "Auto9" : "Auto"; state.applied = false; renderAll(); }
    });
  }
}

function renderQuality() {
  $("quality").innerHTML = QUALITY_KEYS.map((key) => `
    <label class="choice" title="${t(`q.${key}.desc`)}"><input type="radio" name="quality" value="${key}" ${state.form.quality === key ? "checked" : ""} />
      <span class="choice-text"><span class="choice-title">${t(`q.${key}.title`)}</span><span class="choice-meta">${t(`q.${key}.meta`)}</span></span></label>`).join("");
  for (const input of document.querySelectorAll('input[name="quality"]')) {
    input.addEventListener("change", () => { state.form.quality = input.value; state.applied = false; state.sent = false; state.error = null; renderBar(); });
  }
}

let openSelect = false;
function setOpen(open, focusButton = false) {
  const list = $("size-list"), button = $("size-btn");
  openSelect = open;
  list.hidden = !open;
  button.setAttribute("aria-expanded", String(open));
  if (open) (list.querySelector('[aria-selected="true"]') || list.querySelector(".select-item"))?.focus();
  else if (focusButton) button.focus();
}

function renderSize() {
  openSelect = false;
  const sizes = state.form.page_sizes?.length ? state.form.page_sizes : [state.form.page_size];
  const groups = new Map();
  for (const key of sizes) { const g = sizeInfo(key).group; groups.set(g, [...(groups.get(g) || []), key]); }
  const list = [...groups].map(([group, items]) => `<div role="group" aria-label="${t(group)}"><div class="select-group" aria-hidden="true">${t(group)}</div>${items.map((key) =>
    `<button class="select-item" type="button" role="option" tabindex="-1" data-key="${key}" aria-selected="${key === state.form.page_size}"><span>${sizeText(key)}</span>${key === state.form.page_size ? icon("check") : ""}</button>`).join("")}</div>`).join("");
  $("size").innerHTML = `<button class="select-btn" type="button" id="size-btn" aria-haspopup="listbox" aria-expanded="false" aria-labelledby="h-size size-btn"><span>${sizeText(state.form.page_size)}</span>${icon("chevron")}</button><div class="select-list" role="listbox" aria-labelledby="h-size" id="size-list" hidden>${list}</div>`;
  $("size-btn").addEventListener("click", () => setOpen(!openSelect));
  for (const item of document.querySelectorAll(".select-item")) {
    item.addEventListener("click", () => { state.form.page_size = item.dataset.key; state.applied = false; renderAll(); $("size-btn").focus(); });
  }
}

/** Keyboard for the size picker: arrows open and move, Home/End jump, Tab and Escape close. */
function onSelectKey(event) {
  if (event.target.id === "size-btn") {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); setOpen(true); }
    return;
  }
  const items = [...document.querySelectorAll(".select-item")];
  const at = items.indexOf(document.activeElement);
  if (at < 0) return;
  const move = { ArrowDown: Math.min(at + 1, items.length - 1), ArrowUp: Math.max(at - 1, 0), Home: 0, End: items.length - 1 }[event.key];
  if (move !== undefined) { event.preventDefault(); items[move].focus(); }
  else if (event.key === "Tab") setOpen(false, true); // focus the button, then Tab moves on from it
}

function renderHostError() {
  const box = $("host-error");
  box.hidden = !state.hostError;
  box.textContent = state.hostError ? errorText(state.hostError) : "";
  if (state.hostError) $("host").setAttribute("aria-invalid", "true"); else $("host").removeAttribute("aria-invalid");
}

function renderBar() {
  const text = $("bar-text");
  const dirty = isDirty();
  $("apply").disabled = !dirty || state.applying;
  $("apply").toggleAttribute("aria-busy", state.applying);
  $("reset").disabled = state.applying || !state.form || keys.every((k) => state.form[k] === DEFAULTS[k]);
  const st = state.printer?.status;
  $("test").disabled = !state.form || state.applying || state.testing || dirty || !st?.ready || !st?.tape_supported;
  $("test").toggleAttribute("aria-busy", state.testing);
  $("test").title = dirty ? t("test.dirty") : st && !st.ready ? t("test.notready") : "";
  if (state.testing) { text.dataset.tone = ""; text.textContent = t("test.sending"); }
  else if (state.error) { text.dataset.tone = "bad"; text.textContent = state.error; }
  else if (dirty) { text.dataset.tone = "dirty"; text.textContent = t("bar.dirty"); }
  else if (state.sent) { text.dataset.tone = "ok"; text.textContent = t("test.sent"); }
  else if (state.applied) { text.dataset.tone = "ok"; text.textContent = t("bar.applied"); }
  else { text.dataset.tone = ""; text.textContent = state.saved ? t("bar.using") : ""; }
}

function renderChecks() {
  for (const id of ["half_cut", "full_cut", "mirror"]) $(id).checked = !!state.form[id];
  $("chain").checked = false;
}

function renderAll() {
  const ready = !!state.form;
  $("form").hidden = !ready;
  $("bar").hidden = !ready;
  $("conn").hidden = state.queueError;
  if (ready) { renderChecks(); renderQuality(); renderSize(); }
  renderStatus(); renderAlerts(); renderBar(); renderHostError();
}

// ---------------------------------------------------------------- actions
async function loadPrinter() {
  state.loadingPrinter = true; renderStatus();
  state.printer = await call("get_printer");
  state.loadingPrinter = false;
  if (state.printer?.host && document.activeElement !== $("host")) $("host").value = state.printer.host;
  renderStatus(); renderAlerts(); renderBar();
}

async function loadSettings() {
  try {
    const settings = await call("get_settings");
    state.saved = settings;
    state.form = structuredClone(settings);
    state.queueError = false;
  } catch (_) {
    state.queueError = true;
  }
  renderAll();
}

async function apply() {
  state.applying = true; state.error = null; state.applied = false; state.sent = false; renderBar();
  try {
    await call("apply_settings", { settings: state.form });
    state.saved = structuredClone(state.form);
    state.applied = true;
  } catch (error) {
    state.error = t("e.notApplied", { detail: errorText(error) });
  }
  state.applying = false; renderBar();
}

async function testPrint() {
  state.testing = true; state.error = null; state.sent = false; renderBar();
  try {
    await call("test_print");
    state.sent = true;
    // the printer takes a moment to start and finish; look again afterwards
    setTimeout(loadPrinter, 4000);
  } catch (error) {
    state.error = errorText(error);
  }
  state.testing = false; renderBar();
}

async function saveHost() {
  const input = $("host");
  const button = $("save-host");
  state.hostError = null; renderHostError();
  button.setAttribute("aria-busy", "true");
  try {
    await call("set_printer_host", { host: input.value });
    await loadPrinter();
  } catch (error) {
    state.hostError = error;
    renderHostError();
    input.focus();
  }
  button.removeAttribute("aria-busy");
}

function changeLanguage(next) {
  if (next === lang) return;
  setLang(next);
  applyStaticText();
  renderAll();
}

// ---------------------------------------------------------------- wiring
paintStaticIcons();
applyStaticText();
for (const button of document.querySelectorAll("#lang button")) button.addEventListener("click", () => changeLanguage(button.dataset.lang));
for (const id of ["half_cut", "full_cut", "mirror"]) {
  $(id).addEventListener("change", () => { state.form[id] = $(id).checked; state.applied = false; state.error = null; renderBar(); });
}
$("apply").addEventListener("click", apply);
$("test").addEventListener("click", testPrint);
$("reset").addEventListener("click", () => { Object.assign(state.form, DEFAULTS); state.applied = false; renderAll(); });
$("save-host").addEventListener("click", saveHost);
$("host").addEventListener("input", () => { if (state.hostError) { state.hostError = null; renderHostError(); } });
$("host").addEventListener("keydown", (event) => { if (event.key === "Enter") saveHost(); });
$("size").addEventListener("keydown", onSelectKey);
document.addEventListener("click", (event) => { if (openSelect && !event.target.closest("#size")) setOpen(false); });
document.addEventListener("keydown", (event) => { if (event.key === "Escape" && openSelect) setOpen(false, true); });

renderStatus();
$("form").hidden = true; $("bar").hidden = true;
loadSettings();
loadPrinter();
