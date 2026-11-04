// Native Phase 0 diagnostic surface. No secret entry or input injection.
// Visual language mirrors the Android/Apple Phase 0 "Liquid Glass" screens:
// luminous teal backdrop, frosted dark panels with a reflected rim, pill glass controls.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <windowsx.h>
#include <commctrl.h>
#include <dwmapi.h>
#include <uxtheme.h>
#include <algorithm>
#include <array>
#include <string>
#include <string_view>
using std::max; // GDI+ headers expect min/max in the global namespace.
using std::min;
#include <objidl.h>
#include <gdiplus.h>
#include "diagnostic-runner.h"

namespace {
namespace gp = Gdiplus;
constexpr int id_inventory = 101, id_ble = 102, id_stop = 103, id_details = 104;
constexpr DWORD dwm_dark_mode_attribute = 20; // DWMWA_USE_IMMERSIVE_DARK_MODE

gp::Color argb(int a, int r, int g, int b) {
    return gp::Color(static_cast<BYTE>(a), static_cast<BYTE>(r), static_cast<BYTE>(g), static_cast<BYTE>(b));
}
gp::RectF to_rect(const RECT& r) {
    return gp::RectF(static_cast<float>(r.left), static_cast<float>(r.top),
                     static_cast<float>(r.right - r.left), static_cast<float>(r.bottom - r.top));
}
void round_path(gp::GraphicsPath& path, gp::RectF r, float radius) {
    radius = (std::min)(radius, (std::min)(r.Width, r.Height) / 2.0f);
    const float d = (std::max)(radius * 2.0f, 1.0f);
    path.AddArc(r.X, r.Y, d, d, 180, 90);
    path.AddArc(r.GetRight() - d, r.Y, d, d, 270, 90);
    path.AddArc(r.GetRight() - d, r.GetBottom() - d, d, d, 0, 90);
    path.AddArc(r.X, r.GetBottom() - d, d, d, 90, 90);
    path.CloseFigure();
}
void glow(gp::Graphics& g, float cx, float cy, float radius, gp::Color center) {
    gp::GraphicsPath path;
    path.AddEllipse(cx - radius, cy - radius, radius * 2, radius * 2);
    gp::PathGradientBrush brush(&path);
    brush.SetCenterColor(center);
    const gp::Color edge(0, center.GetR(), center.GetG(), center.GetB());
    INT count = 1;
    brush.SetSurroundColors(&edge, &count);
    g.FillPath(&brush, &path);
}
void draw_backdrop(gp::Graphics& g, float w, float h) {
    gp::LinearGradientBrush base(gp::RectF(0, 0, w, h), argb(255, 8, 25, 43), argb(255, 23, 35, 66),
                                 gp::LinearGradientModeForwardDiagonal);
    const gp::Color colors[3] = {argb(255, 8, 25, 43), argb(255, 14, 62, 72), argb(255, 23, 35, 66)};
    const gp::REAL positions[3] = {0.0f, 0.5f, 1.0f};
    base.SetInterpolationColors(colors, positions, 3);
    g.FillRectangle(&base, 0.0f, 0.0f, w, h);
    gp::GraphicsState state = g.Save();
    g.SetClip(gp::RectF(0, 0, w, h));
    glow(g, w * 0.9f, h * 0.12f, w * 0.8f, argb(135, 50, 190, 175));
    glow(g, w * 0.05f, h * 0.8f, w * 0.9f, argb(85, 99, 111, 213));
    g.Restore(state);
}
// Frosted dark glass with a bright upper-left rim, as in GlassSurface.kt.
void draw_glass(gp::Graphics& g, gp::RectF r, float radius, float rim_width) {
    gp::GraphicsPath path;
    round_path(path, r, radius);
    gp::LinearGradientBrush fill(r, argb(242, 32, 65, 78), argb(237, 14, 35, 52), gp::LinearGradientModeForwardDiagonal);
    g.FillPath(&fill, &path);
    gp::GraphicsState state = g.Save();
    g.SetClip(&path, gp::CombineModeIntersect);
    glow(g, r.X + r.Width * 0.2f, r.Y, r.Width, argb(42, 174, 255, 233));
    g.Restore(state);
    gp::RectF inner = r;
    inner.Inflate(-rim_width / 2.0f, -rim_width / 2.0f);
    gp::GraphicsPath rim;
    round_path(rim, inner, radius - rim_width / 2.0f);
    gp::LinearGradientBrush rim_brush(r, argb(170, 226, 255, 249), argb(80, 126, 217, 221), gp::LinearGradientModeForwardDiagonal);
    const gp::Color rim_colors[3] = {argb(170, 226, 255, 249), argb(20, 214, 255, 247), argb(80, 126, 217, 221)};
    const gp::REAL rim_positions[3] = {0.0f, 0.6f, 1.0f};
    rim_brush.SetInterpolationColors(rim_colors, rim_positions, 3);
    gp::Pen pen(&rim_brush, rim_width);
    g.DrawPath(&pen, &rim);
}

struct App {
    HWND window{}, title{}, subtitle{}, tag{}, action_title{}, action_hint{};
    HWND inventory{}, ble{}, stop{}, result_title{}, result{}, safety{}, details{}, log{}, footer{};
    HFONT heading_font{}, body_font{}, small_font{}, log_font{};
    HBRUSH log_brush{};
    diagnostic_gui::ProbeRunner runner;
    diagnostic_gui::Action action = diagnostic_gui::Action::Inventory;
    bool busy = false, expanded = false, high_contrast = false;
    UINT dpi = 96;
    RECT action_card{}, result_card{}, log_card{};
    App() { log_brush = CreateSolidBrush(log_fill()); }
    ~App() {
        for (auto font : {heading_font, body_font, small_font, log_font}) if (font) DeleteObject(font);
        if (log_brush) DeleteObject(log_brush);
    }
    int px(int value) const { return MulDiv(value, static_cast<int>(dpi), 96); }
    // Palette: light-on-glass normally, system colors in Windows high-contrast mode.
    COLORREF ink() const { return high_contrast ? GetSysColor(COLOR_WINDOWTEXT) : RGB(244, 251, 255); }
    COLORREF muted() const { return high_contrast ? GetSysColor(COLOR_WINDOWTEXT) : RGB(176, 208, 216); }
    COLORREF tag_color() const { return high_contrast ? GetSysColor(COLOR_WINDOWTEXT) : RGB(126, 217, 221); }
    COLORREF log_fill() const { return high_contrast ? GetSysColor(COLOR_WINDOW) : RGB(16, 40, 56); }
    COLORREF log_text() const { return high_contrast ? GetSysColor(COLOR_WINDOWTEXT) : RGB(214, 238, 242); }
    HWND label(const wchar_t* text, DWORD style = 0) {
        return CreateWindowExW(WS_EX_TRANSPARENT, L"STATIC", text, WS_CHILD | WS_VISIBLE | style,
            0, 0, 0, 0, window, nullptr, GetModuleHandleW(nullptr), nullptr);
    }
    HWND button(const wchar_t* text, int id) {
        return CreateWindowExW(0, L"BUTTON", text, WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW,
            0, 0, 0, 0, window, reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)), GetModuleHandleW(nullptr), nullptr);
    }
    void fonts() {
        const auto make = [this](int height, int weight, const wchar_t* face) {
            return CreateFontW(-px(height), 0, 0, 0, weight, FALSE, FALSE, FALSE, DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH, face);
        };
        const std::array<HFONT, 4> old{heading_font, body_font, small_font, log_font};
        heading_font = make(34, FW_BOLD, L"Segoe UI");
        body_font = make(16, FW_NORMAL, L"Segoe UI");
        small_font = make(13, FW_NORMAL, L"Segoe UI");
        log_font = make(12, FW_NORMAL, L"Consolas");
        for (auto child : {subtitle, action_title, inventory, ble, stop, result_title, result, details})
            SendMessageW(child, WM_SETFONT, reinterpret_cast<WPARAM>(body_font), TRUE);
        for (auto child : {tag, action_hint, safety, footer})
            SendMessageW(child, WM_SETFONT, reinterpret_cast<WPARAM>(small_font), TRUE);
        SendMessageW(title, WM_SETFONT, reinterpret_cast<WPARAM>(heading_font), TRUE);
        SendMessageW(log, WM_SETFONT, reinterpret_cast<WPARAM>(log_font), TRUE);
        for (auto font : old) if (font) DeleteObject(font);
    }
    // Transparent statics sit on a painted backdrop, so the parent must repaint with them.
    void repaint() {
        RedrawWindow(window, nullptr, nullptr, RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN);
    }
    void set_text(HWND control, const wchar_t* text) {
        SetWindowTextW(control, text);
        RECT rect{}; GetWindowRect(control, &rect);
        MapWindowPoints(nullptr, window, reinterpret_cast<POINT*>(&rect), 2);
        RedrawWindow(window, &rect, nullptr, RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN);
    }
    void update_contrast() {
        HIGHCONTRASTW contrast{sizeof(contrast), 0, nullptr};
        high_contrast = SystemParametersInfoW(SPI_GETHIGHCONTRAST, sizeof(contrast), &contrast, 0) &&
            (contrast.dwFlags & HCF_HIGHCONTRASTON) != 0;
        if (log_brush) DeleteObject(log_brush);
        log_brush = CreateSolidBrush(log_fill());
        const BOOL dark = high_contrast ? FALSE : TRUE;
        DwmSetWindowAttribute(window, dwm_dark_mode_attribute, &dark, sizeof(dark));
        if (log) SetWindowTheme(log, high_contrast ? L"Explorer" : L"DarkMode_Explorer", nullptr);
    }
    void layout() {
        RECT client{}; GetClientRect(window, &client);
        const int width = client.right, margin = px(32), inner = width - margin * 2;
        const auto move = [](HWND control, int x, int y, int w, int h) {
            MoveWindow(control, x, y, (std::max)(w, 1), (std::max)(h, 1), TRUE);
        };
        move(tag, margin, px(25), inner, px(22));
        move(title, margin, px(50), inner, px(52));
        move(subtitle, margin, px(110), inner, px(32));
        action_card = {margin, px(162), width - margin, px(326)};
        move(action_title, margin + px(24), px(182), inner - px(48), px(28));
        move(action_hint, margin + px(24), px(216), inner - px(48), px(24));
        const int gap = px(12), available = inner - px(48), stop_width = px(90);
        const int action_width = (available - stop_width - gap * 2) / 2;
        move(inventory, margin + px(24), px(258), action_width, px(46));
        move(ble, margin + px(24) + action_width + gap, px(258), action_width, px(46));
        move(stop, width - margin - px(24) - stop_width, px(258), stop_width, px(46));
        const int result_bottom = (std::max)(px(500), static_cast<int>(client.bottom) - px(124) - (expanded ? px(150) : 0));
        result_card = {margin, px(344), width - margin, result_bottom};
        move(result_title, margin + px(24), px(366), inner - px(48), px(28));
        move(result, margin + px(24), px(408), inner - px(48), result_bottom - px(428));
        move(details, margin, result_bottom + px(12), px(220), px(36));
        if (expanded) {
            log_card = {margin, result_bottom + px(56), width - margin, client.bottom - px(72)};
            move(log, log_card.left + px(18), log_card.top + px(16), inner - px(36),
                 log_card.bottom - log_card.top - px(32));
        } else log_card = {};
        ShowWindow(log, expanded ? SW_SHOW : SW_HIDE);
        move(safety, margin, client.bottom - px(60), inner, px(24));
        move(footer, margin, client.bottom - px(32), inner, px(20));
        repaint();
    }
    // Backdrop and glass cards. Owner-drawn buttons call this with an offset so their
    // rounded corners blend into whatever lies underneath.
    void paint_scene(gp::Graphics& g, int w, int h) const {
        if (high_contrast) {
            gp::SolidBrush bg(gp::Color(255, GetRValue(GetSysColor(COLOR_WINDOW)), GetGValue(GetSysColor(COLOR_WINDOW)),
                                        GetBValue(GetSysColor(COLOR_WINDOW))));
            g.FillRectangle(&bg, 0, 0, w, h);
            const COLORREF edge = GetSysColor(COLOR_WINDOWTEXT);
            gp::Pen pen(gp::Color(255, GetRValue(edge), GetGValue(edge), GetBValue(edge)), static_cast<float>(px(2)));
            for (const RECT* card : {&action_card, &result_card, &log_card})
                if (card->right > card->left) g.DrawRectangle(&pen, to_rect(*card));
            return;
        }
        draw_backdrop(g, static_cast<float>(w), static_cast<float>(h));
        const float rim = (std::max)(1.0f, static_cast<float>(px(1)));
        for (const RECT* card : {&action_card, &result_card, &log_card})
            if (card->right > card->left) draw_glass(g, to_rect(*card), static_cast<float>(px(28)), rim);
    }
    bool has(std::string_view value) const { return runner.Output().find(value) != std::string::npos; }
    void buttons() {
        EnableWindow(inventory, !busy); EnableWindow(ble, !busy); EnableWindow(stop, busy);
        for (auto control : {inventory, ble, stop}) InvalidateRect(control, nullptr, FALSE);
    }
    void result_text(const wchar_t* heading, const wchar_t* text) {
        set_text(result_title, heading); set_text(result, text);
    }
    void refresh_log() {
        // Child emits public diagnostic tokens only. Do not display arbitrary free text.
        std::wstring text;
        const auto& output = runner.Output();
        size_t start = 0;
        while (start < output.size() && text.size() < 4096) {
            const size_t end = output.find('\n', start);
            std::string_view line(output.data() + start, (end == std::string::npos ? output.size() : end) - start);
            if (!line.empty() && line.back() == '\r') line.remove_suffix(1);
            const bool safe = line.size() <= 120 && !line.empty() && std::all_of(line.begin(), line.end(), [](char ch) {
                return (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9') || ch == '_' || ch == '=' || ch == 'x';
            });
            if (safe) { for (char ch : line) text += static_cast<wchar_t>(ch); text += L"\r\n"; }
            if (end == std::string::npos) break;
            start = end + 1;
        }
        SetWindowTextW(log, text.empty() ? L"진단 결과가 여기에 표시됩니다." : text.c_str());
    }
    void finish() {
        busy = false; KillTimer(window, 1); buttons(); refresh_log();
        if (runner.Cancelled()) result_text(L"시험을 중지했습니다", L"진행 중인 진단을 종료했습니다. 버튼을 눌러 새 시험을 시작할 수 있습니다.");
        else if (runner.TimedOut()) result_text(L"응답 시간이 초과되었습니다", L"진단을 종료했습니다. 라디오와 시험 송신기 상태를 확인한 뒤 다시 시작하세요.");
        else if (!runner.Error().empty()) result_text(L"진단을 실행할 수 없습니다", runner.Error().c_str());
        else if (has("REFUSED_NOT_NORMAL_MEDIUM_INTEGRITY") || has("DIAGNOSTIC_ONLY_NOT_NORMAL_USER_FEASIBILITY"))
            result_text(L"일반 사용자 환경이 필요합니다", L"일반 데스크톱에서 실행하세요. 관리자 권한으로 실행하거나 보안 설정을 변경하지 마세요.");
        else if (action == diagnostic_gui::Action::Inventory && runner.ExitCode() == 0)
            result_text(L"BLE 수신 기능을 확인했습니다", has("TPM_PROVIDER_OPENED_NOT_KEY_VERIFIED")
                ? L"BLE LE · Central 기능이 보고되었습니다.\r\nTPM 제공자 접근도 확인했지만, TPM 2.0과 보호된 키 사용은 아직 검증하지 않았습니다."
                : L"BLE LE · Central 기능이 보고되었습니다.\r\nTPM 제공자를 사용할 수 없습니다. 하드웨어 보호 여부는 아직 검증하지 않았습니다.");
        else if (has("NO_DIAGNOSTIC_SENDER"))
            result_text(L"시험용 송신기를 찾지 못했습니다", L"10초 검색을 마쳤습니다. Android 또는 Apple 시험판을 전면에서 광고한 뒤 다시 시도하세요.");
        else if (has("AMBIGUOUS_SENDERS"))
            result_text(L"시험용 송신기가 여러 대입니다", L"다른 시험판의 광고를 중지하고, 한 대의 송신기만 남긴 뒤 다시 시작하세요.");
        else if (runner.ExitCode() == 0 && has("SYNTHETIC_GATT_WRITE_READ_PASS"))
            result_text(L"공개 문자열 교환을 확인했습니다", L"고정된 시험 문자열의 쓰기와 읽기가 일치했습니다.\r\n상대 신원 · 암호화 · 실제 비밀번호 전송 · 자동 입력을 검증한 결과는 아닙니다.");
        else result_text(L"진단을 완료하지 못했습니다", L"Bluetooth 라디오·권한·어댑터·송신기 상태를 확인하세요. 아래 상세 결과에서 진단 코드를 볼 수 있습니다.");
    }
    void start(diagnostic_gui::Action selected) {
        if (busy) return;
        action = selected;
        if (!runner.Start(action)) { finish(); return; }
        busy = true; buttons();
        result_text(action == diagnostic_gui::Action::Inventory ? L"어댑터를 확인하고 있습니다" : L"시험용 송신기를 검색하고 있습니다",
            action == diagnostic_gui::Action::Inventory ? L"읽기 전용 BLE 기능과 TPM 제공자 접근을 확인합니다." : L"약 10초 동안 공개 시험 서비스만 검색합니다. 전체 시험은 최대 60초입니다.");
        refresh_log();
        if (!SetTimer(window, 1, 100, nullptr)) { runner.Cancel(); finish(); }
    }
};

LRESULT CALLBACK readonly_log(HWND control, UINT message, WPARAM wp, LPARAM lp, UINT_PTR, DWORD_PTR) {
    if (message == WM_COPY || message == WM_CUT || message == WM_PASTE || message == WM_CONTEXTMENU) return 0;
    return DefSubclassProc(control, message, wp, lp);
}

// Pill-shaped glass control. Primary action gets a teal tint; disabled controls dim.
void draw_button(const App& app, const DRAWITEMSTRUCT& item) {
    const int w = item.rcItem.right - item.rcItem.left, h = item.rcItem.bottom - item.rcItem.top;
    if (w <= 0 || h <= 0) return;
    const bool enabled = (item.itemState & ODS_DISABLED) == 0;
    const bool pressed = (item.itemState & ODS_SELECTED) != 0;
    const bool primary = item.CtlID == id_inventory;
    const bool focus = (item.itemState & ODS_FOCUS) != 0 && (item.itemState & ODS_NOFOCUSRECT) == 0;
    wchar_t caption[96]{}; GetWindowTextW(item.hwndItem, caption, 96);

    const HDC memory = CreateCompatibleDC(item.hDC);
    const HBITMAP bitmap = CreateCompatibleBitmap(item.hDC, w, h);
    const HGDIOBJ old_bitmap = SelectObject(memory, bitmap);
    {
        gp::Graphics g(memory);
        g.SetSmoothingMode(gp::SmoothingModeAntiAlias);
        g.SetPixelOffsetMode(gp::PixelOffsetModeHalf);
        POINT origin{0, 0}; MapWindowPoints(item.hwndItem, app.window, &origin, 1);
        RECT parent{}; GetClientRect(app.window, &parent);
        g.TranslateTransform(static_cast<float>(-origin.x), static_cast<float>(-origin.y));
        app.paint_scene(g, parent.right, parent.bottom);
        g.ResetTransform();
        const gp::RectF rect(0.0f, 0.0f, static_cast<float>(w), static_cast<float>(h));
        if (app.high_contrast) {
            const COLORREF face = GetSysColor(COLOR_BTNFACE), edge = GetSysColor(COLOR_BTNTEXT);
            gp::SolidBrush fill(gp::Color(255, GetRValue(face), GetGValue(face), GetBValue(face)));
            g.FillRectangle(&fill, rect);
            gp::Pen pen(gp::Color(255, GetRValue(edge), GetGValue(edge), GetBValue(edge)), static_cast<float>(app.px(2)));
            g.DrawRectangle(&pen, rect.X + 1, rect.Y + 1, rect.Width - 2, rect.Height - 2);
        } else {
            const float rim = (std::max)(1.0f, static_cast<float>(app.px(1)));
            draw_glass(g, rect, rect.Height / 2.0f, rim);
            gp::GraphicsPath path; round_path(path, rect, rect.Height / 2.0f);
            if (primary && enabled) { gp::SolidBrush tint(argb(78, 45, 205, 185)); g.FillPath(&tint, &path); }
            if (pressed && enabled) { gp::SolidBrush press(argb(70, 0, 0, 0)); g.FillPath(&press, &path); }
            if (!enabled) { gp::SolidBrush dim(argb(90, 8, 20, 32)); g.FillPath(&dim, &path); }
        }
        if (focus) {
            const COLORREF ring = app.high_contrast ? GetSysColor(COLOR_HIGHLIGHT) : RGB(226, 255, 249);
            gp::Pen pen(gp::Color(255, GetRValue(ring), GetGValue(ring), GetBValue(ring)), static_cast<float>(app.px(2)));
            gp::GraphicsPath outline;
            gp::RectF inset = rect; inset.Inflate(-static_cast<float>(app.px(4)), -static_cast<float>(app.px(4)));
            round_path(outline, inset, inset.Height / 2.0f);
            g.DrawPath(&pen, &outline);
        }
    }
    COLORREF text = app.ink();
    if (app.high_contrast) text = GetSysColor(enabled ? COLOR_BTNTEXT : COLOR_GRAYTEXT);
    else if (!enabled) text = RGB(118, 146, 156);
    SetBkMode(memory, TRANSPARENT); SetTextColor(memory, text);
    const HGDIOBJ old_font = SelectObject(memory, app.body_font);
    RECT text_rect{0, 0, w, h};
    DrawTextW(memory, caption, -1, &text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    SelectObject(memory, old_font);
    BitBlt(item.hDC, item.rcItem.left, item.rcItem.top, w, h, memory, 0, 0, SRCCOPY);
    SelectObject(memory, old_bitmap); DeleteObject(bitmap); DeleteDC(memory);
}

LRESULT CALLBACK procedure(HWND window, UINT message, WPARAM wp, LPARAM lp) {
    auto* app = reinterpret_cast<App*>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        app = static_cast<App*>(reinterpret_cast<CREATESTRUCTW*>(lp)->lpCreateParams);
        app->window = window; SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(app));
    }
    if (!app) return DefWindowProcW(window, message, wp, lp);
    switch (message) {
    case WM_CREATE: {
        app->dpi = GetDpiForWindow(window);
        app->tag = app->label(L"AreaTM  /  WINDOWS RECEIVER  /  PHASE 0");
        app->title = app->label(L"KagiLink");
        app->subtitle = app->label(L"가까운 기기와 BLE 연결을 확인하세요.");
        app->action_title = app->label(L"연결 시험");
        app->action_hint = app->label(L"송신기는 광고하고, 이 컴퓨터는 검색합니다.");
        app->inventory = app->button(L"어댑터 확인 (&A)", id_inventory);
        app->ble = app->button(L"BLE 연결 시험 (&B)", id_ble);
        app->stop = app->button(L"중지 (&S)", id_stop);
        app->result_title = app->label(L"시험 준비가 되었습니다");
        app->result = app->label(L"먼저 어댑터를 확인하세요.\r\nBLE 시험에는 전면에서 광고 중인 Android 또는 Apple 시험판 한 대가 필요합니다.");
        app->details = app->button(L"상세 결과 보기 (&D)", id_details);
        app->log = CreateWindowExW(0, L"EDIT", L"진단 결과가 여기에 표시됩니다.",
            WS_CHILD | WS_TABSTOP | ES_MULTILINE | ES_READONLY | WS_VSCROLL,
            0, 0, 0, 0, window, nullptr, GetModuleHandleW(nullptr), nullptr);
        SetWindowSubclass(app->log, readonly_log, 1, 0);
        app->safety = app->label(L"공개 고정 문자열 전용 · 실제 비밀번호 전송 및 RDP 입력 비활성");
        app->footer = app->label(L"상대 기기 신원과 하드웨어 보호는 아직 검증하지 않은 연구용 시험판입니다.");
        if (!app->inventory || !app->ble || !app->stop || !app->log || !app->result || !app->details) return -1;
        app->update_contrast();
        app->fonts(); app->buttons(); app->layout();
        return 0;
    }
    case WM_SIZE: app->layout(); return 0;
    case WM_SETTINGCHANGE: case WM_SYSCOLORCHANGE:
        app->update_contrast(); app->repaint(); return 0;
    case WM_DPICHANGED: {
        app->dpi = HIWORD(wp); app->fonts();
        const auto* rect = reinterpret_cast<const RECT*>(lp);
        SetWindowPos(window, nullptr, rect->left, rect->top, rect->right - rect->left, rect->bottom - rect->top, SWP_NOZORDER | SWP_NOACTIVATE);
        app->layout(); return 0;
    }
    case WM_GETMINMAXINFO: {
        auto* info = reinterpret_cast<MINMAXINFO*>(lp);
        info->ptMinTrackSize = {app->px(700), app->px(740)}; return 0;
    }
    case WM_COMMAND:
        if (HIWORD(wp) != BN_CLICKED) break;
        switch (LOWORD(wp)) {
        case id_inventory: app->start(diagnostic_gui::Action::Inventory); return 0;
        case id_ble: app->start(diagnostic_gui::Action::Ble); return 0;
        case id_stop: app->runner.Cancel(); app->finish(); return 0;
        case id_details:
            app->expanded = !app->expanded;
            SetWindowTextW(app->details, app->expanded ? L"상세 결과 숨기기 (&D)" : L"상세 결과 보기 (&D)");
            app->layout(); return 0;
        }
        break;
    case WM_TIMER:
        if (wp == 1 && app->busy) { app->runner.Poll(); if (!app->runner.Running()) app->finish(); }
        return 0;
    case WM_CTLCOLOREDIT: case WM_CTLCOLORSTATIC: {
        const auto dc = reinterpret_cast<HDC>(wp); const auto control = reinterpret_cast<HWND>(lp);
        if (control == app->log) { // Read-only edits report as static.
            SetTextColor(dc, app->log_text()); SetBkColor(dc, app->log_fill());
            return reinterpret_cast<LRESULT>(app->log_brush);
        }
        COLORREF color = app->muted();
        if (control == app->tag) color = app->tag_color();
        else if (control == app->title || control == app->subtitle || control == app->action_title ||
                 control == app->result_title || control == app->result) color = app->ink();
        SetTextColor(dc, color); SetBkMode(dc, TRANSPARENT);
        return reinterpret_cast<LRESULT>(GetStockObject(NULL_BRUSH));
    }
    case WM_DRAWITEM: {
        const auto* item = reinterpret_cast<DRAWITEMSTRUCT*>(lp);
        if (item->CtlType != ODT_BUTTON) break;
        draw_button(*app, *item);
        return TRUE;
    }
    case WM_ERASEBKGND: return 1;
    case WM_PAINT: {
        PAINTSTRUCT paint{}; const auto dc = BeginPaint(window, &paint);
        RECT client{}; GetClientRect(window, &client);
        const HDC memory = CreateCompatibleDC(dc);
        const HBITMAP bitmap = CreateCompatibleBitmap(dc, client.right, client.bottom);
        const HGDIOBJ old_bitmap = SelectObject(memory, bitmap);
        {
            gp::Graphics g(memory);
            g.SetSmoothingMode(gp::SmoothingModeAntiAlias);
            g.SetPixelOffsetMode(gp::PixelOffsetModeHalf);
            app->paint_scene(g, client.right, client.bottom);
        }
        BitBlt(dc, 0, 0, client.right, client.bottom, memory, 0, 0, SRCCOPY);
        SelectObject(memory, old_bitmap); DeleteObject(bitmap); DeleteDC(memory);
        EndPaint(window, &paint); return 0;
    }
    case WM_CLOSE: app->runner.Cancel(); DestroyWindow(window); return 0;
    case WM_DESTROY: KillTimer(window, 1); app->runner.Cancel(); PostQuitMessage(0); return 0;
    }
    return DefWindowProcW(window, message, wp, lp);
}
} // namespace

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE, PWSTR command, int show) {
    if (command && *command) return 2; // This GUI accepts no command/payload arguments.
    InitCommonControls();
    gp::GdiplusStartupInput startup;
    ULONG_PTR gdiplus_token = 0;
    if (gp::GdiplusStartup(&gdiplus_token, &startup, nullptr) != gp::Ok) return 2;
    int exit_code = 2;
    {
        App app;
        WNDCLASSW wc{}; wc.lpfnWndProc = procedure; wc.hInstance = instance;
        wc.lpszClassName = L"KagiLink_Phase0_Receiver"; wc.hCursor = LoadCursorW(nullptr, IDC_ARROW);
        wc.hIcon = LoadIconW(nullptr, IDI_APPLICATION);
        if (RegisterClassW(&wc)) {
            // No WS_CLIPCHILDREN: transparent labels rely on the parent's glass backdrop.
            const auto window = CreateWindowExW(WS_EX_CONTROLPARENT, wc.lpszClassName, L"KagiLink · Windows BLE 진단",
                WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT, 940, 820, nullptr, nullptr, instance, &app);
            if (window) {
                const DWM_WINDOW_CORNER_PREFERENCE corner = DWMWCP_ROUND;
                DwmSetWindowAttribute(window, DWMWA_WINDOW_CORNER_PREFERENCE, &corner, sizeof(corner));
                ShowWindow(window, show); UpdateWindow(window); SetFocus(app.inventory);
                MSG message{}; BOOL status{};
                while ((status = GetMessageW(&message, nullptr, 0, 0)) > 0) {
                    if (!IsDialogMessageW(window, &message)) { TranslateMessage(&message); DispatchMessageW(&message); }
                }
                exit_code = status == -1 ? 2 : static_cast<int>(message.wParam);
            }
        }
    }
    gp::GdiplusShutdown(gdiplus_token);
    return exit_code;
}
