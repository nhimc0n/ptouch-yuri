// SPDX-License-Identifier: GPL-3.0-or-later
//
// All user-facing text, Vietnamese and English. Keys are shared; {name} is a
// placeholder. Technical details that come from the system (an error message
// from the printer, an address) stay as they are.

const I18N = {
  vi: {
    "lang.label": "Ngôn ngữ",
    "head.title": "PT-E850TKW",

    "opt.h": "Tuỳ chọn in",
    "half.title": "Cắt nửa",
    "half.desc": "Cắt qua lớp nhãn nhưng giữ giấy lót. Bật cả hai kiểu cắt thì cắt nửa được ưu tiên, tắt cả hai thì máy không cắt.",
    "full.title": "Cắt đứt hẳn",
    "full.desc": "Cắt rời cả nhãn lẫn giấy lót sau mỗi lần in.",

    "mirror.title": "In lật chữ",
    "mirror.desc": "Dùng khi đọc nhãn xuyên từ mặt sau, ví dụ nhãn trong suốt dán lên kính.",
    "chain.title": "In liền",
    "chain.tag": "Chưa có",
    "chain.desc": "Không cắt giữa các nhãn. Cần ghi lại cách máy nhận nhiều nhãn liền nhau, sẽ có ở bản sau.",

    "quality.h": "Chất lượng in",
    "q.Normal.title": "Thường",
    "q.Normal.meta": "360 × 360 dpi",
    "q.Normal.desc": "Nhanh, đủ rõ cho hầu hết nhãn.",
    "q.High.title": "Cao",
    "q.High.meta": "chậm hơn",
    "q.High.desc": "In chậm hơn để nét đều hơn, cùng độ phân giải.",
    "q.HiRes.title": "Độ phân giải cao",
    "q.HiRes.meta": "360 × 720 dpi",
    "q.HiRes.desc": "Chậm nhất, mịn hơn theo chiều dọc băng. Chỉ dùng cho băng TZe phủ nhựa.",

    "size.h": "Khổ nhãn mặc định",
    "size.g.auto": "Tự động theo nội dung",
    "size.g.l36": "Cố định, băng 36 mm, nhãn ngang",
    "size.g.p36": "Cố định, băng 36 mm, nhãn dọc",
    "size.g.s9": "Cố định, băng 9 mm",
    "size.g.other": "Khác",
    "size.Auto": "Tự động theo nội dung, nhãn ngang",
    "size.AutoP": "Tự động theo nội dung, nhãn dọc",
    "size.Auto9": "Tự động theo nội dung",
    "size.tape": "băng {mm} mm",

    "conn.h": "Địa chỉ máy in",
    "conn.save": "Lưu",
    "conn.hint": "Địa chỉ IP của máy in trong mạng. Nên đặt IP cố định trên router.",

    "bar.dirty": "Có thay đổi chưa áp dụng",
    "bar.applied": "Đã áp dụng. Có tác dụng từ lần in tiếp theo, ở mọi ứng dụng.",
    "bar.using": "Đang dùng cài đặt này.",
    "bar.apply": "Áp dụng",
    "bar.reset": "Khôi phục mặc định",
    "test.btn": "In thử",
    "test.dirty": "Áp dụng thay đổi trước khi in thử",
    "test.notready": "Máy in chưa sẵn sàng",
    "test.sending": "Đang gửi nhãn thử…",
    "test.sent": "Đã gửi nhãn thử đến máy in.",

    "st.idle": "Sẵn sàng in",
    "st.printing": "Máy đang in",
    "st.error": "Máy báo lỗi",
    "st.warmup": "Máy đang khởi động",
    "st.unknown": "Chưa rõ trạng thái",
    "st.noconn": "Không kết nối được máy in",
    "st.address": "Địa chỉ {host}",
    "st.noaddress": "Chưa có địa chỉ máy in",
    "st.tape": "Băng {mm} mm",
    "st.refresh": "Làm mới trạng thái",

    "b.noqueue.t": "Chưa cài driver",
    "b.noqueue.d": "Không tìm thấy hàng đợi in PT-E850TKW. Cài driver bằng scripts/install-cups-macos.sh rồi mở lại app.",
    "b.noconn.t": "Không kết nối được máy in",
    "b.noconn.d": "Kiểm tra máy in đã bật và cùng mạng với máy tính, hoặc sửa địa chỉ ở mục Kết nối. Chi tiết: {detail}.",
    "b.noconn.none": "không có phản hồi",
    "b.retry": "Thử lại",
    "b.error.t": "Máy in đang báo lỗi",
    "b.error.d": "Xem thông báo trên màn hình máy in, bấm Huỷ trên máy hoặc mở nắp rồi đóng lại. Xong thì bấm Làm mới.",
    "b.refresh": "Làm mới",
    "b.unsupported.t": "Băng {mm} mm chưa được hỗ trợ",
    "b.unsupported.d": "Hiện chỉ in được băng TZe 9 mm và 36 mm.",
    "b.mismatch.t": "Băng đang lắp là {have} mm, khổ mặc định dành cho băng {want} mm",
    "b.mismatch.d": "In bằng khổ này máy sẽ từ chối lệnh in.",
    "b.mismatch.a": "Dùng khổ {have} mm",

    "e.notApplied": "Không áp dụng được: {detail}",
    "e.noQueue": "Chưa tìm thấy hàng đợi in PT-E850TKW. Hãy cài driver trước.",
    "e.notReady": "Máy in chưa sẵn sàng (trạng thái: {state}). Kiểm tra máy rồi thử lại.",
    "e.noTestLabel": "Chưa có nhãn thử cho băng {mm} mm.",
    "e.sendFail": "Không gửi được nhãn thử: {detail}",
    "e.badHost": "Nhập địa chỉ IP hoặc tên máy của máy in, ví dụ 192.168.99.107.",
  },
  en: {
    "lang.label": "Language",
    "head.title": "PT-E850TKW",

    "opt.h": "Print options",
    "half.title": "Half cut",
    "half.desc": "Cuts through the label but keeps the backing paper. If both cuts are on, half cut wins; if both are off, the printer does not cut.",
    "full.title": "Full cut",
    "full.desc": "Cuts the label and the backing paper apart after every print.",

    "mirror.title": "Mirror print",
    "mirror.desc": "Use when the label is read from behind, such as a clear label stuck on glass.",
    "chain.title": "Chain printing",
    "chain.tag": "Not yet",
    "chain.desc": "No cut between labels. How the printer receives several labels in a row still has to be recorded; coming later.",

    "quality.h": "Print quality",
    "q.Normal.title": "Normal",
    "q.Normal.meta": "360 × 360 dpi",
    "q.Normal.desc": "Fast and clear enough for most labels.",
    "q.High.title": "High",
    "q.High.meta": "slower",
    "q.High.desc": "Prints slower for more even, sharper output at the same resolution.",
    "q.HiRes.title": "High resolution",
    "q.HiRes.meta": "360 × 720 dpi",
    "q.HiRes.desc": "Slowest, smoother along the tape length. Laminated TZe tape only.",

    "size.h": "Default label size",
    "size.g.auto": "Automatic, fits the content",
    "size.g.l36": "Fixed, 36 mm tape, landscape",
    "size.g.p36": "Fixed, 36 mm tape, portrait",
    "size.g.s9": "Fixed, 9 mm tape",
    "size.g.other": "Other",
    "size.Auto": "Automatic, landscape",
    "size.AutoP": "Automatic, portrait",
    "size.Auto9": "Automatic",
    "size.tape": "{mm} mm tape",

    "conn.h": "Printer address",
    "conn.save": "Save",
    "conn.hint": "The printer's IP address on your network. Set a fixed IP on the router.",

    "bar.dirty": "You have unapplied changes",
    "bar.applied": "Applied. Takes effect on the next print, in every app.",
    "bar.using": "Using these settings.",
    "bar.apply": "Apply",
    "bar.reset": "Restore defaults",
    "test.btn": "Test print",
    "test.dirty": "Apply your changes before a test print",
    "test.notready": "The printer is not ready",
    "test.sending": "Sending the test label…",
    "test.sent": "Test label sent to the printer.",

    "st.idle": "Ready to print",
    "st.printing": "Printing",
    "st.error": "Printer error",
    "st.warmup": "Printer warming up",
    "st.unknown": "Status unknown",
    "st.noconn": "Cannot reach the printer",
    "st.address": "Address {host}",
    "st.noaddress": "No printer address",
    "st.tape": "{mm} mm tape",
    "st.refresh": "Refresh status",

    "b.noqueue.t": "Driver not installed",
    "b.noqueue.d": "The print queue PT-E850TKW was not found. Install the driver with scripts/install-cups-macos.sh and reopen the app.",
    "b.noconn.t": "Cannot reach the printer",
    "b.noconn.d": "Check that the printer is on and on the same network as this computer, or fix the address under Connection. Details: {detail}.",
    "b.noconn.none": "no response",
    "b.retry": "Try again",
    "b.error.t": "The printer reports an error",
    "b.error.d": "Look at the printer's display, press Cancel on it or open and close the cover. Then press Refresh.",
    "b.refresh": "Refresh",
    "b.unsupported.t": "{mm} mm tape is not supported yet",
    "b.unsupported.d": "Only 9 mm and 36 mm TZe tape can be printed for now.",
    "b.mismatch.t": "Loaded tape is {have} mm but the default size is for {want} mm tape",
    "b.mismatch.d": "With this size the printer will reject the print job.",
    "b.mismatch.a": "Use {have} mm size",

    "e.notApplied": "Could not apply: {detail}",
    "e.noQueue": "The print queue PT-E850TKW was not found. Install the driver first.",
    "e.notReady": "The printer is not ready (state: {state}). Check it and try again.",
    "e.noTestLabel": "There is no test label for {mm} mm tape.",
    "e.sendFail": "Could not send the test label: {detail}",
    "e.badHost": "Enter the printer's IP address or host name, for example 192.168.99.107.",
  },
};

let lang = "vi";
try {
  const saved = localStorage.getItem("lang");
  if (saved === "vi" || saved === "en") lang = saved;
} catch (_) { /* storage can be unavailable; keep the default */ }

function t(key, vars = {}) {
  const text = I18N[lang][key] ?? I18N.vi[key] ?? key;
  return text.replace(/\{(\w+)\}/g, (_, name) => vars[name] ?? "");
}

/** Show a backend error in the current language when we know it; otherwise as is. */
function errorText(error) {
  const raw = String(error);
  if (/was not found/i.test(raw)) return t("e.noQueue");
  if (/Enter the printer's IP/i.test(raw)) return t("e.badHost");
  let m;
  if ((m = raw.match(/printer is not ready \(state: (\w+)\)/i))) return t("e.notReady", { state: m[1] });
  if ((m = raw.match(/test label is not available for (\d+) mm/i))) return t("e.noTestLabel", { mm: m[1] });
  if ((m = raw.match(/Could not send the test label: (.*)/is))) return t("e.sendFail", { detail: m[1] });
  return raw;
}

function setLang(next) {
  lang = next;
  try { localStorage.setItem("lang", next); } catch (_) { /* ignore */ }
}

function applyStaticText() {
  document.documentElement.lang = lang;
  for (const el of document.querySelectorAll("[data-i18n]")) el.textContent = t(el.dataset.i18n);
  for (const el of document.querySelectorAll("[data-i18n-title]")) el.title = t(el.dataset.i18nTitle);
  for (const el of document.querySelectorAll("[data-i18n-aria]")) el.setAttribute("aria-label", t(el.dataset.i18nAria));
  for (const button of document.querySelectorAll("#lang button")) button.setAttribute("aria-pressed", String(button.dataset.lang === lang));
}
