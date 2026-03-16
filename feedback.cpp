// 命令行入口 - AI 直接调用，启动 GUI 并等待用户反馈
// 用法: feedback.exe "工作摘要" [超时秒数]
// stdout 输出用户反馈内容，超时或关闭窗口则输出空行
#include <windows.h>
#include <shellapi.h>
#include <string>
#include <cstdio>
#include "common.h"

int main()
{
    SetConsoleCP(CP_UTF8);
    SetConsoleOutputCP(CP_UTF8);

    int argc = 0;
    LPWSTR* wargv = CommandLineToArgvW(GetCommandLineW(), &argc);
    if (!wargv || argc < 2) {
        fprintf(stderr, "Usage: feedback.exe \"summary\" [timeout_seconds]\n");
        if (wargv)
            LocalFree(wargv);
        return 1;
    }

    std::string summary = wide_to_utf8(wargv[1]);

    WCHAR exe_path[MAX_PATH];
    GetModuleFileNameW(nullptr, exe_path, MAX_PATH);
    fs::path exe_dir = fs::path(exe_path).parent_path();

    int timeout_sec = 600;
    if (argc >= 3)
        timeout_sec = _wtoi(wargv[2]);
    LocalFree(wargv);
    if (timeout_sec <= 0)
        timeout_sec = 600;

    fs::path temp_file = make_temp_path("feedback_direct");
    std::error_code ec;
    fs::remove(temp_file, ec);

    HANDLE gui_process = nullptr;
    DWORD gui_pid = 0;
    if (!gui_launch(exe_dir, summary, temp_file, gui_process, gui_pid)) {
        fprintf(stderr, "Failed to launch feedback-gui.exe\n");
        return 1;
    }

    DWORD wait = WaitForSingleObject(gui_process, (DWORD)timeout_sec * 1000);

    if (wait == WAIT_TIMEOUT) {
        HWND hwnd = find_window_by_pid(gui_pid);
        if (hwnd)
            PostMessage(hwnd, WM_FEEDBACK_TIMEOUT, 0, 0);
        CloseHandle(gui_process);
        printf("\n");
        return 0;
    }

    CloseHandle(gui_process);

    std::string feedback = read_and_delete(temp_file);
    printf("%s\n", feedback.c_str());
    return 0;
}
