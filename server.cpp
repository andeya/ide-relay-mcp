// MCP 服务端 - 通过 stdio 与 Cursor 进行 JSON-RPC 通信
#include <windows.h>
#include <string>
#include <vector>
#include <fstream>
#include <chrono>
#include <ctime>
#include <filesystem>
#include "json.hpp"

using json = nlohmann::json;
namespace fs = std::filesystem;

// 与 gui.cpp 保持一致的自定义消息
static const UINT WM_FEEDBACK_SUPERSEDED = WM_USER + 100;
static const UINT WM_FEEDBACK_CANCELLED  = WM_USER + 101;

// ============================================================
// 全局状态
// ============================================================

struct AutoReplyRule {
    int timeout_seconds;
    std::string text;
};

struct ServerState {
    fs::path exe_dir;
    fs::path temp_file;
    HANDLE stdout_handle = INVALID_HANDLE_VALUE;

    HANDLE gui_process = nullptr;
    HWND   gui_hwnd    = nullptr;

    bool pending = false;
    json pending_id;

    std::chrono::steady_clock::time_point wait_start;
    int loop_index = 0;
};

static ServerState g;

// ============================================================
// 编码转换
// ============================================================

static std::wstring utf8_to_wide(const std::string& s)
{
    if (s.empty()) return {};
    int len = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), (int)s.size(), nullptr, 0);
    std::wstring ws(len, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, s.c_str(), (int)s.size(), ws.data(), len);
    return ws;
}

// ============================================================
// JSON-RPC 通信
// ============================================================

static void send_json(const json& j)
{
    std::string line = j.dump() + "\n";
    DWORD written;
    WriteFile(g.stdout_handle, line.c_str(), (DWORD)line.size(), &written, nullptr);
    FlushFileBuffers(g.stdout_handle);
}

static void send_result(const json& id, const json& result)
{
    send_json({{"jsonrpc", "2.0"}, {"id", id}, {"result", result}});
}

static void send_error(const json& id, int code, const std::string& msg)
{
    send_json({{"jsonrpc", "2.0"}, {"id", id},
               {"error", {{"code", code}, {"message", msg}}}});
}

// ============================================================
// 日志
// ============================================================

static void log_interaction(const std::string& source, const std::string& content)
{
    auto t = std::chrono::system_clock::to_time_t(std::chrono::system_clock::now());
    std::tm tm;
    localtime_s(&tm, &t);

    char ts[32];
    std::strftime(ts, sizeof(ts), "%Y-%m-%d %H:%M:%S", &tm);
    std::string line = "[" + std::string(ts) + "] [" + source + "] " + content + "\n";

    HANDLE hFile = CreateFileW(
        (g.exe_dir / "feedback_log.txt").c_str(),
        FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE,
        nullptr, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (hFile == INVALID_HANDLE_VALUE) return;

    DWORD written;
    WriteFile(hFile, line.c_str(), (DWORD)line.size(), &written, nullptr);
    CloseHandle(hFile);
}

// ============================================================
// 自动回复
// ============================================================

// 文件格式：每行 "超时秒数|回复内容"，# 开头的行为注释
static std::vector<AutoReplyRule> load_rules(const fs::path& path)
{
    std::vector<AutoReplyRule> rules;
    std::ifstream in(path);
    if (!in) return rules;

    std::string line;
    while (std::getline(in, line)) {
        if (!line.empty() && line.back() == '\r') line.pop_back();
        if (line.empty() || line[0] == '#') continue;
        auto sep = line.find('|');
        if (sep == std::string::npos) continue;
        try {
            rules.push_back({std::stoi(line.substr(0, sep)), line.substr(sep + 1)});
        } catch (...) {}
    }
    return rules;
}

// 查看下一条自动回复规则，from_oneshot 标识来源
static bool peek_auto_reply(AutoReplyRule& rule, bool& from_oneshot)
{
    auto oneshot = load_rules(g.exe_dir / "auto_reply_oneshot.txt");
    if (!oneshot.empty()) {
        rule = oneshot[0];
        from_oneshot = true;
        return true;
    }

    auto loop = load_rules(g.exe_dir / "auto_reply_loop.txt");
    if (loop.empty()) return false;

    rule = loop[g.loop_index % (int)loop.size()];
    from_oneshot = false;
    return true;
}

// 原子地消费 oneshot 文件的第一条有效规则（跳过注释和空行）
static void consume_oneshot()
{
    HANDLE mutex = CreateMutexW(nullptr, FALSE, L"Global\\FeedbackMCP_Oneshot");
    if (!mutex) return;

    DWORD wait = WaitForSingleObject(mutex, 5000);
    if (wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED) {
        CloseHandle(mutex);
        return;
    }

    fs::path path = g.exe_dir / "auto_reply_oneshot.txt";
    std::ifstream in(path);
    if (in) {
        std::vector<std::string> lines;
        std::string line;
        bool removed = false;
        while (std::getline(in, line)) {
            std::string trimmed = line;
            if (!trimmed.empty() && trimmed.back() == '\r') trimmed.pop_back();
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
            if (i + 1 < lines.size()) out << '\n';
        }
    }

    ReleaseMutex(mutex);
    CloseHandle(mutex);
}

// ============================================================
// GUI 进程管理
// ============================================================

static BOOL CALLBACK find_window_cb(HWND hwnd, LPARAM lp)
{
    auto* out = reinterpret_cast<std::pair<DWORD, HWND>*>(lp);
    DWORD pid = 0;
    GetWindowThreadProcessId(hwnd, &pid);
    if (pid == out->first) { out->second = hwnd; return FALSE; }
    return TRUE;
}

static HWND find_window_by_pid(DWORD pid)
{
    std::pair<DWORD, HWND> data = {pid, nullptr};
    EnumWindows(find_window_cb, reinterpret_cast<LPARAM>(&data));
    return data.second;
}

static std::wstring escape_arg(const std::string& utf8)
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

static bool launch_gui(const std::string& summary)
{
    if (g.gui_hwnd) {
        PostMessage(g.gui_hwnd, WM_FEEDBACK_SUPERSEDED, 0, 0);
        g.gui_hwnd = nullptr;
    }
    if (g.gui_process) {
        CloseHandle(g.gui_process);
        g.gui_process = nullptr;
    }

    // 清理旧临时文件并启动 GUI 进程
    std::error_code ec;
    fs::remove(g.temp_file, ec);

    fs::path gui = g.exe_dir / "feedback_gui.exe";
    std::wstring cmd = L"\"" + gui.wstring() + L"\" "
                     + escape_arg(summary) + L" "
                     + escape_arg(g.temp_file.u8string());

    STARTUPINFOW si = {};
    si.cb = sizeof(si);
    PROCESS_INFORMATION pi = {};

    if (!CreateProcessW(nullptr, cmd.data(), nullptr, nullptr,
                        FALSE, 0, nullptr, nullptr, &si, &pi)) {
        return false;
    }

    CloseHandle(pi.hThread);
    g.gui_process = pi.hProcess;

    for (int i = 0; i < 50 && !g.gui_hwnd; ++i) {
        Sleep(100);
        g.gui_hwnd = find_window_by_pid(pi.dwProcessId);
    }

    g.wait_start = std::chrono::steady_clock::now();
    return true;
}

static std::string read_temp_file()
{
    std::ifstream in(g.temp_file, std::ios::binary);
    if (!in) return "";

    std::string s((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
    in.close();

    while (!s.empty() && (s.back() == '\n' || s.back() == '\r'))
        s.pop_back();

    std::error_code ec;
    fs::remove(g.temp_file, ec);
    return s;
}

// ============================================================
// 请求完成
// ============================================================

static void finish_request(const std::string& feedback, const std::string& source)
{
    if (!g.pending) return;
    g.pending = false;

    log_interaction(source, feedback);

    json text_item = {
        {"type", "text"},
        {"text", json({{"interactive_feedback", feedback}}).dump()}
    };
    send_result(g.pending_id, {{"content", json::array({text_item})}});
}

// ============================================================
// MCP 消息处理
// ============================================================

static void handle_initialize(const json& msg)
{
    send_result(msg["id"], {
        {"protocolVersion", "2024-11-05"},
        {"capabilities", {{"tools", json::object()}}},
        {"serverInfo", {{"name", "interactive-feedback-mcp"}, {"version", "1.0.0"}}}
    });
}

static void handle_tools_list(const json& msg)
{
    json tool;
    tool["name"] = "interactive_feedback";
    tool["description"] = "Pause and wait for user feedback before proceeding.";
    tool["inputSchema"] = {
        {"type", "object"},
        {"properties", {{"summary", {{"type", "string"}, {"description", "Summary of work done"}}}}},
        {"required", json::array({"summary"})}
    };
    send_result(msg["id"], {{"tools", json::array({tool})}});
}

static void handle_tools_call(const json& msg)
{
    std::string name = msg["params"]["name"];
    if (name != "interactive_feedback") {
        send_error(msg["id"], -32601, "Unknown tool: " + name);
        return;
    }

    std::string summary = msg["params"]["arguments"].value("summary", "");
    log_interaction("AI_REQUEST", summary);

    g.pending_id = msg["id"];
    g.pending = true;

    AutoReplyRule rule;
    bool from_oneshot;
    if (peek_auto_reply(rule, from_oneshot) && rule.timeout_seconds == 0) {
        if (from_oneshot)
            consume_oneshot();
        else
            ++g.loop_index;
        finish_request(rule.text, "AUTO_REPLY");
        return;
    }

    if (!launch_gui(summary)) {
        finish_request("", "ERROR");
        return;
    }
}

static void handle_cancelled(const json& msg)
{
    if (!msg.contains("params") || !msg["params"].contains("requestId")) return;
    if (g.pending && g.pending_id == msg["params"]["requestId"]) {
        g.pending = false;
        if (g.gui_hwnd) {
            PostMessage(g.gui_hwnd, WM_FEEDBACK_CANCELLED, 0, 0);
            g.gui_hwnd = nullptr;
        }
    }
}

static void process_message(const json& msg)
{
    if (!msg.contains("method")) return;

    std::string method = msg["method"];

    // 通知（无 id）
    if (!msg.contains("id")) {
        if (method == "notifications/cancelled") handle_cancelled(msg);
        return;
    }

    // 请求（有 id）
    if (method == "initialize")  { handle_initialize(msg); return; }
    if (method == "ping")        { send_result(msg["id"], json::object()); return; }
    if (method == "tools/list")  { handle_tools_list(msg); return; }
    if (method == "tools/call")  { handle_tools_call(msg); return; }

    send_error(msg["id"], -32601, "Method not found: " + method);
}

// ============================================================
// stdin 异步读取线程
// ============================================================

struct StdinReader {
    HANDLE event;
    CRITICAL_SECTION cs;
    std::vector<std::string> lines;
    bool eof = false;
};

static DWORD WINAPI stdin_thread(LPVOID param)
{
    auto* ctx = static_cast<StdinReader*>(param);
    HANDLE stdin_handle = GetStdHandle(STD_INPUT_HANDLE);
    char buf[4096];
    std::string partial;

    while (true) {
        DWORD bytes_read = 0;
        if (!ReadFile(stdin_handle, buf, sizeof(buf), &bytes_read, nullptr) ||
            bytes_read == 0) {
            EnterCriticalSection(&ctx->cs);
            ctx->eof = true;
            LeaveCriticalSection(&ctx->cs);
            SetEvent(ctx->event);
            break;
        }

        partial.append(buf, bytes_read);

        std::vector<std::string> new_lines;
        size_t pos;
        while ((pos = partial.find('\n')) != std::string::npos) {
            std::string line = partial.substr(0, pos);
            if (!line.empty() && line.back() == '\r') line.pop_back();
            if (!line.empty()) new_lines.push_back(std::move(line));
            partial.erase(0, pos + 1);
        }

        if (!new_lines.empty()) {
            EnterCriticalSection(&ctx->cs);
            for (auto& l : new_lines) ctx->lines.push_back(std::move(l));
            LeaveCriticalSection(&ctx->cs);
            SetEvent(ctx->event);
        }
    }
    return 0;
}

// ============================================================
// 主循环
// ============================================================

int main()
{
    SetConsoleCP(CP_UTF8);
    SetConsoleOutputCP(CP_UTF8);

    WCHAR exe_path[MAX_PATH];
    GetModuleFileNameW(nullptr, exe_path, MAX_PATH);
    g.exe_dir = fs::path(exe_path).parent_path();
    g.stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);

    WCHAR temp_dir[MAX_PATH];
    GetTempPathW(MAX_PATH, temp_dir);
    g.temp_file = fs::path(temp_dir) / ("feedback_mcp_" + std::to_string(GetCurrentProcessId()) + ".tmp");

    StdinReader reader;
    reader.event = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    InitializeCriticalSection(&reader.cs);
    CreateThread(nullptr, 0, stdin_thread, &reader, 0, nullptr);

    while (true) {
        HANDLE handles[2] = { reader.event };
        DWORD count = 1;
        DWORD timeout = INFINITE;
        AutoReplyRule auto_rule;
        bool auto_oneshot = false;
        bool waiting_gui = g.pending && g.gui_process;

        if (waiting_gui) {
            handles[count++] = g.gui_process;
            if (peek_auto_reply(auto_rule, auto_oneshot)) {
                auto elapsed = std::chrono::duration_cast<std::chrono::seconds>(
                    std::chrono::steady_clock::now() - g.wait_start).count();
                int remaining = auto_rule.timeout_seconds - (int)elapsed;
                timeout = (remaining > 0) ? (DWORD)(remaining * 1000) : 0;
            }
        }

        DWORD result = WaitForMultipleObjects(count, handles, FALSE, timeout);

        // 无论哪个事件触发，都先处理 stdin 队列（避免丢消息）
        std::vector<std::string> batch;
        EnterCriticalSection(&reader.cs);
        batch.swap(reader.lines);
        bool eof = reader.eof;
        LeaveCriticalSection(&reader.cs);

        for (auto& line : batch) {
            try { process_message(json::parse(line)); }
            catch (...) {}
        }
        if (eof) break;

        if (result == WAIT_OBJECT_0 + 1) {
            // GUI 进程退出
            std::string feedback = read_temp_file();
            CloseHandle(g.gui_process);
            g.gui_process = nullptr;
            g.gui_hwnd = nullptr;

            if (!feedback.empty()) g.loop_index = 0;
            finish_request(feedback, feedback.empty() ? "USER_EMPTY" : "USER_REPLY");
        } else if (result == WAIT_TIMEOUT && g.pending) {
            if (auto_oneshot)
                consume_oneshot();
            else
                ++g.loop_index;
            if (g.gui_hwnd) {
                PostMessage(g.gui_hwnd, WM_FEEDBACK_SUPERSEDED, 0, 0);
                g.gui_hwnd = nullptr;
            }
            finish_request(auto_rule.text, "AUTO_REPLY");
        }
    }

    if (g.gui_process) {
        TerminateProcess(g.gui_process, 0);
        CloseHandle(g.gui_process);
    }
    return 0;
}
