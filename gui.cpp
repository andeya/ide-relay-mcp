// GUI 窗口进程 - 显示摘要、收集用户反馈
#include <windows.h>
#include <shellapi.h>
#include "common.h"

// ============================================================
// 全局状态
// ============================================================

struct GuiState {
    std::wstring summary;
    fs::path     result_file;

    HWND  hwnd_main          = nullptr;
    HWND  hwnd_summary_label = nullptr;
    HWND  hwnd_summary       = nullptr;
    HWND  hwnd_input_label   = nullptr;
    HWND  hwnd_edit          = nullptr;
    HWND  hwnd_submit        = nullptr;
    HFONT font               = nullptr;
    bool  expired            = false;
};

static GuiState g;
static WNDPROC g_orig_edit_proc = nullptr;

// ============================================================
// 工具函数
// ============================================================

static int scale(int value)
{
    static int dpi = 0;
    if (dpi == 0) {
        HDC hdc = GetDC(nullptr);
        dpi = GetDeviceCaps(hdc, LOGPIXELSY);
        ReleaseDC(nullptr, hdc);
    }
    return MulDiv(value, dpi, 96);
}

static void insert_file_paths(HDROP hdrop, HWND target)
{
    UINT count = DragQueryFileW(hdrop, 0xFFFFFFFF, nullptr, 0);
    for (UINT i = 0; i < count; ++i) {
        UINT len = DragQueryFileW(hdrop, i, nullptr, 0);
        std::wstring path(len, L'\0');
        DragQueryFileW(hdrop, i, &path[0], len + 1);
        if (i > 0)
            SendMessageW(target, EM_REPLACESEL, TRUE, (LPARAM)L"\r\n");
        SendMessageW(target, EM_REPLACESEL, TRUE, (LPARAM)path.c_str());
    }
}

static void submit_and_close()
{
    int len = GetWindowTextLengthW(g.hwnd_edit);
    std::wstring text(len, L'\0');
    if (len > 0)
        GetWindowTextW(g.hwnd_edit, &text[0], len + 1);

    std::string utf8 = wide_to_utf8(text);
    std::ofstream out(g.result_file, std::ios::binary | std::ios::trunc);
    if (out)
        out.write(utf8.c_str(), (std::streamsize)utf8.size());

    DestroyWindow(g.hwnd_main);
}

// ============================================================
// 布局：固定元素扣除后，剩余空间平分给摘要和输入框
// ============================================================

static void layout(int client_w, int client_h)
{
    const int margin   = scale(12);
    const int gap      = scale(4);
    const int label_h  = scale(20);
    const int button_h = scale(32);
    const int button_w = scale(160);

    int content_w = client_w - margin * 2;

    // 固定高度：两个标签 + 标签间距 + 按钮 + 间距
    int fixed_h = label_h * 2 + gap * 2 + button_h + margin * 4;
    int flex_h = client_h - fixed_h;

    int summary_h = flex_h / 4;
    int edit_h    = flex_h - summary_h;

    int y = margin;

    MoveWindow(g.hwnd_summary_label, margin, y, content_w, label_h, TRUE);
    y += label_h + gap;

    MoveWindow(g.hwnd_summary, margin, y, content_w, summary_h, TRUE);
    y += summary_h + margin;

    MoveWindow(g.hwnd_input_label, margin, y, content_w, label_h, TRUE);
    y += label_h + gap;

    MoveWindow(g.hwnd_edit, margin, y, content_w, edit_h, TRUE);
    y += edit_h + margin;

    MoveWindow(g.hwnd_submit, client_w - margin - button_w, y, button_w, button_h, TRUE);
}

// ============================================================
// 编辑框子类 - Ctrl+Enter 提交、文件粘贴
// ============================================================

static LRESULT CALLBACK edit_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp)
{
    if (msg == WM_KEYDOWN && wp == VK_RETURN &&
        (GetKeyState(VK_CONTROL) & 0x8000) && !g.expired) {
        submit_and_close();
        return 0;
    }
    if (msg == WM_PASTE && IsClipboardFormatAvailable(CF_HDROP)) {
        if (!OpenClipboard(hwnd))
            return 0;
        HDROP hDrop = (HDROP)GetClipboardData(CF_HDROP);
        if (hDrop)
            insert_file_paths(hDrop, hwnd);
        CloseClipboard();
        return 0;
    }
    return CallWindowProcW(g_orig_edit_proc, hwnd, msg, wp, lp);
}

// ============================================================
// 窗口过程
// ============================================================

static LRESULT CALLBACK wnd_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp)
{
    switch (msg) {

    case WM_CREATE: {
        g.hwnd_main = hwnd;

        NONCLIENTMETRICSW ncm = {};
        ncm.cbSize = sizeof(ncm);
        SystemParametersInfoW(SPI_GETNONCLIENTMETRICS, sizeof(ncm), &ncm, 0);
        g.font = CreateFontIndirectW(&ncm.lfMessageFont);

        auto create_ctrl = [&](const wchar_t* cls, const wchar_t* text,
                               DWORD style, DWORD ex_style = 0) {
            HWND h = CreateWindowExW(
                ex_style, cls, text, WS_CHILD | WS_VISIBLE | style,
                0, 0, 0, 0, hwnd, nullptr, nullptr, nullptr
            );
            SendMessageW(h, WM_SETFONT, (WPARAM)g.font, TRUE);
            return h;
        };

        g.hwnd_summary_label = create_ctrl(L"STATIC", L"Summary:", 0);
        g.hwnd_summary = create_ctrl(L"EDIT", g.summary.c_str(),
            WS_VSCROLL | ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL,
            WS_EX_CLIENTEDGE);

        g.hwnd_input_label = create_ctrl(L"STATIC", L"Feedback:", 0);
        g.hwnd_edit = create_ctrl(L"EDIT", L"",
            WS_VSCROLL | WS_TABSTOP | ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN,
            WS_EX_CLIENTEDGE);

        g_orig_edit_proc = (WNDPROC)SetWindowLongPtrW(
            g.hwnd_edit, GWLP_WNDPROC, (LONG_PTR)edit_proc
        );

        g.hwnd_submit = create_ctrl(L"BUTTON", L"Send (Ctrl+Enter)",
            WS_TABSTOP | BS_DEFPUSHBUTTON);

        DragAcceptFiles(hwnd, TRUE);

        RECT rc;
        GetClientRect(hwnd, &rc);
        layout(rc.right, rc.bottom);
        SetFocus(g.hwnd_edit);
        return 0;
    }

    case WM_SIZE:
        layout(LOWORD(lp), HIWORD(lp));
        return 0;

    case WM_COMMAND:
        if (HIWORD(wp) == BN_CLICKED && (HWND)lp == g.hwnd_submit)
            submit_and_close();
        return 0;

    case WM_CTLCOLORSTATIC: {
        HDC hdc = (HDC)wp;
        SetTextColor(hdc, GetSysColor(COLOR_WINDOWTEXT));
        SetBkColor(hdc, GetSysColor(COLOR_WINDOW));
        return (LRESULT)GetSysColorBrush(COLOR_WINDOW);
    }

    case WM_DROPFILES: {
        HDROP hDrop = (HDROP)wp;
        insert_file_paths(hDrop, g.hwnd_edit);
        DragFinish(hDrop);
        return 0;
    }

    case WM_FEEDBACK_CANCELLED:
    case WM_FEEDBACK_TIMEOUT:
        if (GetWindowTextLengthW(g.hwnd_edit) == 0) {
            DestroyWindow(hwnd);
        } else {
            g.expired = true;
            if (msg == WM_FEEDBACK_CANCELLED)
                SetWindowTextW(hwnd, L"Interactive Feedback MCP [Cancelled]");
            else
                SetWindowTextW(hwnd, L"Interactive Feedback MCP [Timed Out]");
            SetWindowTextW(g.hwnd_submit, L"Close");
        }
        return 0;

    case WM_CLOSE:
        DestroyWindow(hwnd);
        return 0;

    case WM_DESTROY:
        if (g.font)
            DeleteObject(g.font);
        PostQuitMessage(0);
        return 0;
    }

    return DefWindowProcW(hwnd, msg, wp, lp);
}

// ============================================================
// 入口
// ============================================================

int WINAPI wWinMain(HINSTANCE hInst, HINSTANCE, LPWSTR, int nShow)
{
    int argc = 0;
    LPWSTR* argv = CommandLineToArgvW(GetCommandLineW(), &argc);
    if (!argv || argc < 3)
        return 1;
    g.summary     = argv[1];
    g.result_file = argv[2];
    LocalFree(argv);

    WNDCLASSEXW wc = {};
    wc.cbSize        = sizeof(wc);
    wc.lpfnWndProc   = wnd_proc;
    wc.hInstance     = hInst;
    wc.hCursor       = LoadCursor(nullptr, IDC_ARROW);
    wc.hbrBackground = (HBRUSH)(COLOR_WINDOW + 1);
    wc.lpszClassName = L"FeedbackGuiClass";
    if (!RegisterClassExW(&wc))
        return 1;

    int win_w = scale(580);
    int win_h = scale(380);
    int screen_w = GetSystemMetrics(SM_CXSCREEN);
    int screen_h = GetSystemMetrics(SM_CYSCREEN);

    CreateWindowExW(
        WS_EX_TOPMOST,
        L"FeedbackGuiClass", L"Interactive Feedback MCP",
        WS_OVERLAPPEDWINDOW,
        (screen_w - win_w) / 2, (screen_h - win_h) / 2, win_w, win_h,
        nullptr, nullptr, hInst, nullptr
    );
    if (!g.hwnd_main)
        return 1;

    ShowWindow(g.hwnd_main, nShow);

    MSG msg;
    while (GetMessageW(&msg, nullptr, 0, 0)) {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    return (int)msg.wParam;
}
