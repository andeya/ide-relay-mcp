#pragma once
#include <windows.h>
#include <string>
#include <fstream>
#include <filesystem>
#include <vector>

namespace fs = std::filesystem;

static const UINT WM_FEEDBACK_TIMEOUT   = WM_USER + 100;
static const UINT WM_FEEDBACK_CANCELLED = WM_USER + 101;

// ============================================================
// 编码转换
// ============================================================

inline std::wstring utf8_to_wide(const std::string& s)
{
    if (s.empty())
        return {};
    int len = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), (int)s.size(), nullptr, 0);
    std::wstring ws(len, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, s.c_str(), (int)s.size(), ws.data(), len);
    return ws;
}

inline std::string wide_to_utf8(const std::wstring& ws)
{
    if (ws.empty())
        return {};
    int len = WideCharToMultiByte(CP_UTF8, 0, ws.c_str(), (int)ws.size(),
                                  nullptr, 0, nullptr, nullptr);
    std::string s(len, '\0');
    WideCharToMultiByte(CP_UTF8, 0, ws.c_str(), (int)ws.size(),
                        s.data(), len, nullptr, nullptr);
    return s;
}

// ============================================================
// 命令行参数转义
// ============================================================

inline std::wstring escape_arg(const std::string& utf8)
{
    std::wstring ws = utf8_to_wide(utf8);
    std::wstring out = L"\"";
    size_t backslashes = 0;
    for (wchar_t ch : ws) {
        if (ch == L'\\') {
            ++backslashes;
        } else if (ch == L'"') {
            out.append(backslashes * 2 + 1, L'\\');
            out += L'"';
            backslashes = 0;
        } else {
            out.append(backslashes, L'\\');
            out += ch;
            backslashes = 0;
        }
    }
    out.append(backslashes * 2, L'\\');
    out += L'"';
    return out;
}

// ============================================================
// 按 PID 查找窗口
// ============================================================

inline BOOL CALLBACK find_window_cb_(HWND hwnd, LPARAM lp)
{
    auto* out = reinterpret_cast<std::pair<DWORD, HWND>*>(lp);
    DWORD pid = 0;
    GetWindowThreadProcessId(hwnd, &pid);
    if (pid == out->first) {
        out->second = hwnd;
        return FALSE;
    }
    return TRUE;
}

inline HWND find_window_by_pid(DWORD pid)
{
    std::pair<DWORD, HWND> data = {pid, nullptr};
    EnumWindows(find_window_cb_, reinterpret_cast<LPARAM>(&data));
    return data.second;
}

// ============================================================
// GUI 启动与结果读取
// ============================================================

inline fs::path make_temp_path(const std::string& prefix = "feedback_mcp")
{
    WCHAR temp_dir[MAX_PATH];
    GetTempPathW(MAX_PATH, temp_dir);

    static LONG counter = 0;
    LONG seq = InterlockedIncrement(&counter);
    return fs::path(temp_dir) / (prefix + "_"
        + std::to_string(GetCurrentProcessId())
        + "_" + std::to_string(seq) + ".tmp");
}

inline bool gui_launch(const fs::path& exe_dir,
                       const std::string& summary,
                       const fs::path& temp_file,
                       HANDLE& out_process, DWORD& out_pid)
{
    fs::path gui = exe_dir / "feedback-gui.exe";
    std::wstring cmd = L"\"" + gui.wstring() + L"\" "
                     + escape_arg(summary) + L" "
                     + escape_arg(temp_file.u8string());

    STARTUPINFOW si = {};
    si.cb = sizeof(si);
    PROCESS_INFORMATION pi = {};

    if (!CreateProcessW(nullptr, cmd.data(), nullptr, nullptr,
                        FALSE, 0, nullptr, nullptr, &si, &pi))
        return false;

    CloseHandle(pi.hThread);
    out_process = pi.hProcess;
    out_pid = pi.dwProcessId;
    return true;
}

inline std::string read_and_delete(const fs::path& path)
{
    std::ifstream in(path, std::ios::binary);
    if (!in)
        return "";

    std::string s((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
    in.close();

    while (!s.empty() && (s.back() == '\n' || s.back() == '\r'))
        s.pop_back();

    std::error_code ec;
    fs::remove(path, ec);
    return s;
}

// ============================================================
// 自动回复
// ============================================================

struct AutoReplyRule {
    int timeout_seconds;
    std::string text;
};

inline std::vector<AutoReplyRule> auto_reply_load(const fs::path& path)
{
    std::vector<AutoReplyRule> rules;
    std::ifstream in(path);
    if (!in)
        return rules;

    std::string line;
    while (std::getline(in, line)) {
        if (!line.empty() && line.back() == '\r')
            line.pop_back();
        if (line.empty() || line[0] == '#')
            continue;
        auto sep = line.find('|');
        if (sep == std::string::npos)
            continue;
        try {
            rules.push_back({std::stoi(line.substr(0, sep)), line.substr(sep + 1)});
        } catch (...) {}
    }
    return rules;
}

inline bool auto_reply_peek(const fs::path& exe_dir, int loop_index,
                            AutoReplyRule& rule, bool& from_oneshot)
{
    auto oneshot = auto_reply_load(exe_dir / "auto_reply_oneshot.txt");
    if (!oneshot.empty()) {
        rule = oneshot[0];
        from_oneshot = true;
        return true;
    }

    auto loop = auto_reply_load(exe_dir / "auto_reply_loop.txt");
    if (loop.empty())
        return false;

    rule = loop[loop_index % (int)loop.size()];
    from_oneshot = false;
    return true;
}

inline void auto_reply_consume_oneshot(const fs::path& exe_dir)
{
    HANDLE mutex = CreateMutexW(nullptr, FALSE, L"Global\\FeedbackMCP_Oneshot");
    if (!mutex)
        return;

    DWORD wait = WaitForSingleObject(mutex, 5000);
    if (wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED) {
        CloseHandle(mutex);
        return;
    }

    fs::path path = exe_dir / "auto_reply_oneshot.txt";
    std::ifstream in(path);
    if (in) {
        std::vector<std::string> lines;
        std::string line;
        bool removed = false;
        while (std::getline(in, line)) {
            std::string trimmed = line;
            if (!trimmed.empty() && trimmed.back() == '\r')
                trimmed.pop_back();
            bool is_rule = !trimmed.empty() && trimmed[0] != '#' && trimmed.find('|') != std::string::npos;
            if (is_rule && !removed) {
                removed = true;
            } else {
                lines.push_back(line);
            }
        }
        in.close();
        std::ofstream out(path, std::ios::trunc | std::ios::binary);
        for (size_t i = 0; i < lines.size(); ++i) {
            out << lines[i];
            if (i + 1 < lines.size())
                out << '\n';
        }
    }

    ReleaseMutex(mutex);
    CloseHandle(mutex);
}
