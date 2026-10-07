// SPDX-License-Identifier: GPL-3.0-or-later
//
// UI only. It never builds printer bytes: it reads and writes the driver
// settings and shows the printer's state through the Tauri commands in
// src-tauri/src/main.rs. Outside Tauri (a plain browser) it runs on made-up
// data so every state can be looked at: ?m=ok|loi|dang-in|mat-ket-noi|chua-cai|tai

const tauri = window.__TAURI__?.core;
const mode = new URLSearchParams(location.search).get("m") || "ok";

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
  if (command === "get_printer") return structuredClone(MOCK.printers[mode] || MOCK.printers.ok);
  return null;
}

// ---------------------------------------------------------------- sizes
const SIZE_LABELS = { Auto: "Tự động theo nội dung, nhãn ngang", AutoP: "Tự động theo nội dung, nhãn dọc", Auto9: "Tự động theo nội dung" };
function sizeInfo(key) {
  const fixed = key.match(/^L(\d+)$/);
  const portrait = key.match(/^P(\d+)$/);
  const nine = key.match(/^S9L(\d+)$/);
  if (key === "Auto" || key === "AutoP") return { group: "Tự động theo nội dung", label: SIZE_LABELS[key], extra: "băng 36 mm", tape: 36 };
  if (key === "Auto9") return { group: "Tự động theo nội dung", label: SIZE_LABELS[key], extra: "băng 9 mm", tape: 9 };
  if (fixed) return { group: "Cố định, băng 36 mm, nhãn ngang", label: `${fixed[1]} × 36 mm`, extra: "", tape: 36 };
  if (portrait) return { group: "Cố định, băng 36 mm, nhãn dọc", label: `36 × ${portrait[1]} mm`, extra: "", tape: 36 };
  if (nine) return { group: "Cố định, băng 9 mm", label: `${nine[1]} × 9 mm`, extra: "", tape: 9 };
  return { group: "Khác", label: key, extra: "", tape: 0 };
}
const sizeText = (key) => { const s = sizeInfo(key); return s.extra ? `${s.label} · ${s.extra}` : s.label; };

const QUALITIES = [
  { key: "Normal", title: "Thường", meta: "360 × 360 dpi", desc: "Nhanh, đủ rõ cho hầu hết nhãn." },
  { key: "High", title: "Chất lượng cao", meta: "chậm hơn", desc: "In chậm hơn để nét đều hơn, cùng độ phân giải." },
  { key: "HiRes", title: "Độ phân giải cao", meta: "360 × 720 dpi", desc: "Chậm nhất, mịn hơn theo chiều dọc băng. Chỉ dùng cho băng TZe phủ nhựa." },
];
const DEFAULTS = { half_cut: true, full_cut: false, chain: false, mirror: false, quality: "Normal", page_size: "Auto" };

// ---------------------------------------------------------------- state
const $ = (id) => document.getElementById(id);
const state = { saved: null, form: null, printer: null, loadingPrinter: true, queueError: null, applied: false, applying: false, error: null };
const keys = ["half_cut", "full_cut", "chain", "mirror", "quality", "page_size"];
const isDirty = () => !!state.saved && keys.some((k) => state.form[k] !== state.saved[k]);

// ---------------------------------------------------------------- render
function renderStatus() {
  const box = $("status");
  const info = state.printer;
  if (state.loadingPrinter && !info) {
    box.dataset.tone = "none";
    box.innerHTML = `<div class="skeleton" style="width:44px;height:44px;border-radius:12px"></div><div class="status-text"><div class="skeleton" style="width:160px;height:20px"></div><div class="skeleton" style="width:240px;height:16px;margin-top:8px"></div></div>`;
    return;
  }
  const st = info?.status;
  let tone = "none", title = "Không kết nối được máy in", sub = info?.host ? `Địa chỉ ${info.host}` : "Chưa có địa chỉ máy in";
  if (st) {
    sub = `${st.model} · ${info.host}`;
    ({ idle: () => { tone = "ok"; title = "Sẵn sàng in"; }, printing: () => { tone = "busy"; title = "Máy đang in"; }, error: () => { tone = "bad"; title = "Máy báo lỗi"; }, warmup: () => { tone = "warn"; title = "Máy đang khởi động"; } }[st.state] || (() => { tone = "warn"; title = "Chưa rõ trạng thái"; }))();
  } else if (info?.error) tone = "bad";
  box.dataset.tone = tone;
  const want = state.form ? sizeInfo(state.form.page_size).tape : 0;
  const tapeOk = st && st.tape_supported && (!want || want === st.tape_mm);
  const tape = st ? `<span class="badge" data-tone="${tapeOk ? "ok" : "warn"}">Băng ${st.tape_mm} mm</span>` : "";
  box.innerHTML = `<div class="status-icon">${icon("printer")}</div>
    <div class="status-text"><div class="status-title">${title}</div><div class="status-sub">${sub}</div></div>
    <div class="status-side">${tape}<button class="icon-btn" id="refresh" type="button" aria-label="Làm mới trạng thái" ${state.loadingPrinter ? 'aria-busy="true"' : ""}>${icon("refresh")}</button></div>`;
  $("refresh").addEventListener("click", loadPrinter);
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
    out.push(banner("bad", "alert", "Chưa cài driver", "Không tìm thấy hàng đợi in PT-E850TKW. Cài driver bằng scripts/install-cups-macos.sh rồi mở lại app."));
  } else if (!state.loadingPrinter && !st) {
    out.push(banner("bad", "alert", "Không kết nối được máy in", `Kiểm tra máy in đã bật và cùng mạng với máy tính, hoặc sửa địa chỉ ở mục Kết nối. Chi tiết: ${info?.error || "không có phản hồi"}.`, { id: "refresh", label: "Thử lại" }));
  } else if (st?.state === "error") {
    out.push(banner("bad", "alert", "Máy in đang báo lỗi", "Xem thông báo trên màn hình máy in, bấm Huỷ trên máy hoặc mở nắp rồi đóng lại. Xong thì bấm Làm mới.", { id: "refresh", label: "Làm mới" }));
  }
  if (st && !st.tape_supported) {
    out.push(banner("warn", "warn", `Băng ${st.tape_mm} mm chưa được hỗ trợ`, "Hiện chỉ in được băng TZe 9 mm và 36 mm."));
  } else if (st && state.form) {
    const want = sizeInfo(state.form.page_size).tape;
    if (want && want !== st.tape_mm) {
      const suggest = st.tape_mm === 9 ? "Auto9" : "Auto";
      out.push(banner("warn", "warn", `Băng đang lắp là ${st.tape_mm} mm, khổ mặc định dành cho băng ${want} mm`, "In bằng khổ này máy sẽ từ chối lệnh in.", state.form.page_sizes?.includes(suggest) ? { id: "fix-size", label: `Dùng khổ ${st.tape_mm} mm` } : null));
    }
  }
  $("alerts").innerHTML = out.join("");
  for (const button of document.querySelectorAll("[data-act]")) {
    button.addEventListener("click", () => {
      if (button.dataset.act === "refresh") loadPrinter();
      if (button.dataset.act === "fix-size") { state.form.page_size = state.printer.status.tape_mm === 9 ? "Auto9" : "Auto"; renderAll(); }
    });
  }
}

function renderQuality() {
  $("quality").innerHTML = `<legend class="sr" hidden>Chất lượng in</legend>` + QUALITIES.map((q) => `
    <label class="choice"><input type="radio" name="quality" value="${q.key}" ${state.form.quality === q.key ? "checked" : ""} />
      <span class="choice-text"><span class="choice-title">${q.title}<span class="choice-meta">${q.meta}</span></span><span class="choice-desc">${q.desc}</span></span></label>`).join("");
  for (const input of document.querySelectorAll('input[name="quality"]')) {
    input.addEventListener("change", () => { state.form.quality = input.value; renderBar(); });
  }
}

let openSelect = false;
function renderSize() {
  const sizes = state.form.page_sizes?.length ? state.form.page_sizes : [state.form.page_size];
  const groups = new Map();
  for (const key of sizes) { const g = sizeInfo(key).group; groups.set(g, [...(groups.get(g) || []), key]); }
  const list = [...groups].map(([name, items]) => `<div role="group" aria-label="${name}"><div class="select-group">${name}</div>${items.map((key) =>
    `<button class="select-item" type="button" role="option" data-key="${key}" aria-selected="${key === state.form.page_size}"><span>${sizeText(key)}</span>${key === state.form.page_size ? icon("check") : ""}</button>`).join("")}</div>`).join("");
  $("size").innerHTML = `<button class="select-btn" type="button" id="size-btn" aria-haspopup="listbox" aria-expanded="${openSelect}"><span>${sizeText(state.form.page_size)}</span>${icon("chevron")}</button><div class="select-list" role="listbox" id="size-list" ${openSelect ? "" : "hidden"}>${list}</div>`;
  $("size-btn").addEventListener("click", (event) => { event.stopPropagation(); openSelect = !openSelect; renderSize(); if (openSelect) $("size-list").querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" }); });
  for (const item of document.querySelectorAll(".select-item")) {
    item.addEventListener("click", () => { state.form.page_size = item.dataset.key; state.applied = false; openSelect = false; renderAll(); });
  }
}

function renderBar() {
  const text = $("bar-text");
  const apply = $("apply");
  const dirty = isDirty();
  apply.disabled = !dirty || state.applying;
  apply.toggleAttribute("aria-busy", state.applying);
  $("reset").disabled = state.applying || !state.form || keys.every((k) => state.form[k] === DEFAULTS[k]);
  if (state.error) { text.dataset.tone = "bad"; text.textContent = state.error; }
  else if (dirty) { text.dataset.tone = "dirty"; text.textContent = "Có thay đổi chưa áp dụng"; }
  else if (state.applied) { text.dataset.tone = "ok"; text.textContent = "Đã áp dụng. Có tác dụng từ lần in tiếp theo, ở mọi ứng dụng."; }
  else { text.dataset.tone = ""; text.textContent = state.saved ? "Đang dùng cài đặt này." : ""; }
}

function renderChecks() {
  for (const id of ["half_cut", "full_cut", "mirror"]) $(id).checked = !!state.form[id];
  $("chain").checked = false;
}

function renderAll() {
  const ready = !!state.form;
  $("form").hidden = !ready;
  $("bar").hidden = !ready;
  $("conn").hidden = !!state.queueError;
  if (ready) { renderChecks(); renderQuality(); renderSize(); }
  renderStatus(); renderAlerts(); renderBar();
}

// ---------------------------------------------------------------- actions
async function loadPrinter() {
  state.loadingPrinter = true; renderStatus();
  state.printer = await call("get_printer");
  state.loadingPrinter = false;
  if (state.printer?.host && document.activeElement !== $("host")) $("host").value = state.printer.host;
  renderStatus(); renderAlerts();
}

async function loadSettings() {
  try {
    const settings = await call("get_settings");
    state.saved = settings;
    state.form = structuredClone(settings);
    state.queueError = null;
  } catch (error) {
    state.queueError = String(error);
  }
  renderAll();
}

async function apply() {
  state.applying = true; state.error = null; state.applied = false; renderBar();
  try {
    await call("apply_settings", { settings: state.form });
    state.saved = structuredClone(state.form);
    state.applied = true;
  } catch (error) {
    state.error = `Không áp dụng được: ${error}`;
  }
  state.applying = false; renderBar();
}

async function saveHost() {
  const input = $("host");
  const button = $("save-host");
  input.removeAttribute("aria-invalid");
  button.setAttribute("aria-busy", "true");
  try {
    await call("set_printer_host", { host: input.value });
    await loadPrinter();
  } catch (error) {
    input.setAttribute("aria-invalid", "true");
    state.error = String(error); renderBar();
  }
  button.removeAttribute("aria-busy");
}

// ---------------------------------------------------------------- wiring
paintStaticIcons();
for (const id of ["half_cut", "full_cut", "mirror"]) {
  $(id).addEventListener("change", () => { state.form[id] = $(id).checked; state.applied = false; state.error = null; renderBar(); });
}
$("quality").addEventListener("change", () => { state.applied = false; state.error = null; });
$("apply").addEventListener("click", apply);
$("reset").addEventListener("click", () => { Object.assign(state.form, DEFAULTS); state.applied = false; renderAll(); });
$("save-host").addEventListener("click", saveHost);
$("host").addEventListener("keydown", (event) => { if (event.key === "Enter") saveHost(); });
document.addEventListener("click", (event) => { if (openSelect && !event.target.closest("#size")) { openSelect = false; renderSize(); } });
document.addEventListener("keydown", (event) => { if (event.key === "Escape" && openSelect) { openSelect = false; renderSize(); $("size-btn").focus(); } });

renderStatus();
$("form").hidden = true; $("bar").hidden = true;
loadSettings();
loadPrinter();
