// MCP 服务端 - 通过 stdio 与 Cursor 进行 JSON-RPC 通信
#include <windows.h>
#include <string>
#include <vector>
#include <ctime>
#include "json.hpp"
#include "common.h"

using json = nlohmann::json;

// ============================================================
// 数据类型
// ============================================================

struct PendingRequest {
    json     id;
    HANDLE   gui_process = nullptr;
    DWORD    gui_pid     = 0;
    fs::path temp_file;
    ULONGLONG created_at = 0;
};

// ============================================================
// 全局状态
// ============================================================

static fs::path g_exe_dir;
static HANDLE   g_stdout = INVALID_HANDLE_VALUE;

static std::vector<PendingRequest> g_pending;
static int g_loop_index = 0;

// ============================================================
// Transport: JSON-RPC 收发
// ============================================================

static void transport_send(const json& j)
{
    std::string line = j.dump() + "\n";
    DWORD written;
    WriteFile(g_stdout, line.c_str(), (DWORD)line.size(), &written, nullptr);
    FlushFileBuffers(g_stdout);
}

static void transport_result(const json& id, const json& result)
{
    transport_send({{"jsonrpc", "2.0"}, {"id", id}, {"result", result}});
}

static void transport_error(const json& id, int code, const std::string& msg)
{
    transport_send({{"jsonrpc", "2.0"}, {"id", id},
                    {"error", {{"code", code}, {"message", msg}}}});
}

struct StdinReader {
    HANDLE event = nullptr;
    CRITICAL_SECTION cs;
    std::vector<std::string> lines;
    bool eof = false;
};

static DWORD WINAPI stdin_thread_proc(LPVOID param)
{
    auto* ctx = static_cast<StdinReader*>(param);
    HANDLE h = GetStdHandle(STD_INPUT_HANDLE);
    char buf[4096];
    std::string partial;

    while (true) {
        DWORD n = 0;
        if (!ReadFile(h, buf, sizeof(buf), &n, nullptr) || n == 0) {
            EnterCriticalSection(&ctx->cs);
            ctx->eof = true;
            LeaveCriticalSection(&ctx->cs);
            SetEvent(ctx->event);
            break;
        }

        partial.append(buf, n);

        std::vector<std::string> new_lines;
        size_t pos;
        while ((pos = partial.find('\n')) != std::string::npos) {
            std::string line = partial.substr(0, pos);
            if (!line.empty() && line.back() == '\r')
                line.pop_back();
            if (!line.empty())
                new_lines.push_back(std::move(line));
            partial.erase(0, pos + 1);
        }

        if (!new_lines.empty()) {
            EnterCriticalSection(&ctx->cs);
            for (auto& l : new_lines)
                ctx->lines.push_back(std::move(l));
            LeaveCriticalSection(&ctx->cs);
            SetEvent(ctx->event);
        }
    }
    return 0;
}

// ============================================================
// 日志
// ============================================================

static void log_write(const std::string& source, const std::string& content)
{
    time_t t = time(nullptr);
    std::tm tm;
    localtime_s(&tm, &t);

    char ts[32];
    std::strftime(ts, sizeof(ts), "%Y-%m-%d %H:%M:%S", &tm);
    std::string line = "[" + std::string(ts) + "] [" + source + "] " + content + "\n";

    HANDLE hFile = CreateFileW(
        (g_exe_dir / "feedback_log.txt").c_str(),
        FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE,
        nullptr, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (hFile == INVALID_HANDLE_VALUE)
        return;

    DWORD written;
    WriteFile(hFile, line.c_str(), (DWORD)line.size(), &written, nullptr);
    CloseHandle(hFile);
}

// ============================================================
// 请求管理
// ============================================================

static PendingRequest* request_find(const json& id)
{
    for (auto& r : g_pending)
        if (r.id == id)
            return &r;
    return nullptr;
}

static void request_respond(PendingRequest& s, const std::string& feedback, const std::string& source)
{
    log_write(source, feedback);

    json text_item = {
        {"type", "text"},
        {"text", json({{"interactive_feedback", feedback}}).dump()}
    };
    transport_result(s.id, {{"content", json::array({text_item})}});
    if (s.gui_process)
        CloseHandle(s.gui_process);
}

static void request_remove(PendingRequest* p)
{
    size_t idx = p - g_pending.data();
    g_pending.erase(g_pending.begin() + idx);
}

// ============================================================
// Protocol: MCP 消息处理
// ============================================================

static void proto_initialize(const json& msg)
{
    transport_result(msg["id"], {
        {"protocolVersion", "2024-11-05"},
        {"capabilities", {{"tools", json::object()}}},
        {"serverInfo", {{"name", "interactive-feedback-mcp"}, {"version", "1.1.0"}}}
    });
}

static void proto_tools_list(const json& msg)
{
    json tool;
    tool["name"] = "interactive_feedback";
    tool["description"] = "Pause and wait for user feedback before proceeding.";
    tool["inputSchema"] = {
        {"type", "object"},
        {"properties", {{"summary", {{"type", "string"}, {"description", "Summary of work done"}}}}},
        {"required", json::array({"summary"})}
    };
    transport_result(msg["id"], {{"tools", json::array({tool})}});
}

static void proto_tools_call(const json& msg)
{
    std::string name = msg["params"]["name"];
    if (name != "interactive_feedback") {
        transport_error(msg["id"], -32601, "Unknown tool: " + name);
        return;
    }

    std::string summary = msg["params"]["arguments"].value("summary", "");
    log_write("AI_REQUEST", summary);

    PendingRequest req;
    req.id = msg["id"];

    AutoReplyRule rule;
    bool from_oneshot;
    if (auto_reply_peek(g_exe_dir, g_loop_index, rule, from_oneshot)
        && rule.timeout_seconds == 0) {
        if (from_oneshot)
            auto_reply_consume_oneshot(g_exe_dir);
        else
            ++g_loop_index;
        request_respond(req, rule.text, "AUTO_REPLY");
        return;
    }

    req.temp_file = make_temp_path();
    std::error_code ec;
    fs::remove(req.temp_file, ec);

    if (!gui_launch(g_exe_dir, summary, req.temp_file, req.gui_process, req.gui_pid)) {
        request_respond(req, "", "ERROR");
        return;
    }

    req.created_at = GetTickCount64();
    g_pending.push_back(std::move(req));
}

static void proto_cancelled(const json& msg)
{
    if (!msg.contains("params") || !msg["params"].contains("requestId"))
        return;
    json target_id = msg["params"]["requestId"];

    PendingRequest* r = request_find(target_id);
    if (!r)
        return;

    if (r->gui_pid) {
        HWND hwnd = find_window_by_pid(r->gui_pid);
        if (hwnd)
            PostMessage(hwnd, WM_FEEDBACK_CANCELLED, 0, 0);
    }
    if (r->gui_process)
        CloseHandle(r->gui_process);
    request_remove(r);
}

static void proto_dispatch(const json& msg)
{
    if (!msg.contains("method"))
        return;

    std::string method = msg["method"];

    if (!msg.contains("id")) {
        if (method == "notifications/cancelled")
            proto_cancelled(msg);
        return;
    }

    if (method == "initialize") {
        proto_initialize(msg);
        return;
    }
    if (method == "ping") {
        transport_result(msg["id"], json::object());
        return;
    }
    if (method == "tools/list") {
        proto_tools_list(msg);
        return;
    }
    if (method == "tools/call") {
        proto_tools_call(msg);
        return;
    }

    transport_error(msg["id"], -32601, "Method not found: " + method);
}

// ============================================================
// 主循环辅助
// ============================================================

static DWORD calc_auto_reply_timeout()
{
    if (g_pending.empty())
        return INFINITE;

    AutoReplyRule rule;
    bool oneshot;
    if (!auto_reply_peek(g_exe_dir, g_loop_index, rule, oneshot))
        return INFINITE;

    int elapsed = (int)((GetTickCount64() - g_pending.front().created_at) / 1000);
    int remaining = rule.timeout_seconds - elapsed;
    return (remaining > 0) ? (DWORD)(remaining * 1000) : 0;
}

static void handle_gui_exit(HANDLE exited)
{
    for (size_t i = 0; i < g_pending.size(); ++i) {
        if (g_pending[i].gui_process == exited) {
            std::string feedback = read_and_delete(g_pending[i].temp_file);
            if (!feedback.empty())
                g_loop_index = 0;
            request_respond(g_pending[i], feedback, "USER_REPLY");
            g_pending.erase(g_pending.begin() + i);
            return;
        }
    }
}

static void handle_auto_reply_timeout()
{
    ULONGLONG now = GetTickCount64();
    while (!g_pending.empty()) {
        AutoReplyRule rule;
        bool oneshot;
        if (!auto_reply_peek(g_exe_dir, g_loop_index, rule, oneshot))
            break;
        auto& r = g_pending.front();
        int elapsed = (int)((now - r.created_at) / 1000);
        if (elapsed < rule.timeout_seconds)
            break;
        if (r.gui_pid) {
            HWND hwnd = find_window_by_pid(r.gui_pid);
            if (hwnd)
                PostMessage(hwnd, WM_FEEDBACK_TIMEOUT, 0, 0);
        }
        request_respond(r, rule.text, "AUTO_REPLY");
        g_pending.erase(g_pending.begin());
        if (oneshot)
            auto_reply_consume_oneshot(g_exe_dir);
        else
            ++g_loop_index;
    }
}

static DWORD WINAPI config_watch_proc(LPVOID param)
{
    HANDLE evt = (HANDLE)param;
    HANDLE notify = FindFirstChangeNotificationW(
        g_exe_dir.c_str(), FALSE, FILE_NOTIFY_CHANGE_LAST_WRITE);
    if (notify == INVALID_HANDLE_VALUE)
        return 0;
    while (WaitForSingleObject(notify, INFINITE) == WAIT_OBJECT_0) {
        SetEvent(evt);
        FindNextChangeNotification(notify);
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
    g_exe_dir = fs::path(exe_path).parent_path();
    g_stdout  = GetStdHandle(STD_OUTPUT_HANDLE);

    StdinReader reader;
    reader.event = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    InitializeCriticalSection(&reader.cs);
    CreateThread(nullptr, 0, stdin_thread_proc, &reader, 0, nullptr);

    HANDLE config_event = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    CreateThread(nullptr, 0, config_watch_proc, config_event, 0, nullptr);

    while (true) {
        HANDLE handles[MAXIMUM_WAIT_OBJECTS];
        handles[0] = reader.event;
        handles[1] = config_event;
        DWORD gui_base = 2;
        DWORD handle_count = gui_base;

        for (auto& s : g_pending) {
            if (handle_count < MAXIMUM_WAIT_OBJECTS)
                handles[handle_count++] = s.gui_process;
        }

        DWORD wait_result = WaitForMultipleObjects(
            handle_count, handles, FALSE, calc_auto_reply_timeout());
        DWORD signaled_idx = wait_result - WAIT_OBJECT_0;
        HANDLE signaled = (signaled_idx < handle_count) ? handles[signaled_idx] : nullptr;

        // ── stdin ──
        std::vector<std::string> batch;
        EnterCriticalSection(&reader.cs);
        batch.swap(reader.lines);
        bool eof = reader.eof;
        LeaveCriticalSection(&reader.cs);

        for (auto& line : batch) {
            try { proto_dispatch(json::parse(line)); }
            catch (...) {}
        }
        if (eof)
            break;

        if (signaled == reader.event) {
            // stdin 已在上面处理
        }
        else if (signaled == config_event || wait_result == WAIT_TIMEOUT) {
            handle_auto_reply_timeout();
        }
        else if (signaled_idx >= gui_base && signaled_idx < handle_count) {
            handle_gui_exit(signaled);
        }
    }

    for (auto& s : g_pending) {
        TerminateProcess(s.gui_process, 0);
        CloseHandle(s.gui_process);
    }

    CloseHandle(reader.event);
    DeleteCriticalSection(&reader.cs);
    return 0;
}
