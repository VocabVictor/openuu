#include <windows.h>
#include <wtsapi32.h>
#include <tlhelp32.h>
#include <comdef.h>
#include <cstdio>
#include <cstdint>
#include <intrin.h>
#include <string>
#include <memory>
#include <shlobj.h> // NOLINT(build/include_order)
#include <userenv.h>
#include <versionhelpers.h>
#include <vector>
#include <sddl.h>
#include <memory>

extern "C" uint32_t get_session_user_info(PWSTR bufin, uint32_t nin, uint32_t id);

extern "C"
{
    static HANDLE thread;
    static DWORD thread_id;

    static HHOOK hook = 0;
    static HWND target_wnd = 0;
    static HWND default_hook_wnd = 0;
    static bool win_down = false;
    static bool stop_system_key_propagate = false;

    bool is_win_down()
    {
        return win_down;
    }

#define ARRAY_SIZE(a) (sizeof(a) / sizeof(*a))

    static int is_system_hotkey(int vkCode, WPARAM wParam)
    {
        switch (vkCode)
        {
        case VK_LWIN:
        case VK_RWIN:
            win_down = wParam == WM_KEYDOWN;
        case VK_SNAPSHOT:
            return 1;
        case VK_TAB:
            if (GetAsyncKeyState(VK_MENU) & 0x8000)
                return 1;
        case VK_ESCAPE:
            if (GetAsyncKeyState(VK_MENU) & 0x8000)
                return 1;
            if (GetAsyncKeyState(VK_CONTROL) & 0x8000)
                return 1;
        }
        return 0;
    }

    static LRESULT CALLBACK keyboard_hook(int nCode, WPARAM wParam, LPARAM lParam)
    {
        if (nCode >= 0)
        {
            KBDLLHOOKSTRUCT *msgInfo = (KBDLLHOOKSTRUCT *)lParam;

            // Grabbing everything seems to mess up some keyboard state that
            // FLTK relies on, so just grab the keys that we normally cannot.
            if (stop_system_key_propagate && is_system_hotkey(msgInfo->vkCode, wParam))
            {
                PostMessage(target_wnd, wParam, msgInfo->vkCode,
                            (msgInfo->scanCode & 0xff) << 16 |
                                (msgInfo->flags & 0xff) << 24);
                return 1;
            }
        }

        return CallNextHookEx(hook, nCode, wParam, lParam);
    }

    static DWORD WINAPI keyboard_thread(LPVOID data)
    {
        MSG msg;

        target_wnd = (HWND)data;

        // Make sure a message queue is created
        PeekMessage(&msg, NULL, 0, 0, PM_NOREMOVE | PM_NOYIELD);

        hook = SetWindowsHookEx(WH_KEYBOARD_LL, keyboard_hook, GetModuleHandle(0), 0);
        // If something goes wrong then there is not much we can do.
        // Just sit around and wait for WM_QUIT...

        while (GetMessage(&msg, NULL, 0, 0))
            ;

        if (hook)
            UnhookWindowsHookEx(hook);

        target_wnd = 0;

        return 0;
    }

    int win32_enable_lowlevel_keyboard(HWND hwnd)
    {
        if (!default_hook_wnd)
        {
            default_hook_wnd = hwnd;
        }
        if (!hwnd)
        {
            hwnd = default_hook_wnd;
        }
        // Only one target at a time for now
        if (thread != NULL)
        {
            if (hwnd == target_wnd)
                return 0;

            return 1;
        }

        // We create a separate thread as it is crucial that hooks are processed
        // in a timely manner.
        thread = CreateThread(NULL, 0, keyboard_thread, hwnd, 0, &thread_id);
        if (thread == NULL)
            return 1;

        return 0;
    }

    void win32_disable_lowlevel_keyboard(HWND hwnd)
    {
        if (!hwnd)
        {
            hwnd = default_hook_wnd;
        }
        if (hwnd != target_wnd)
            return;

        PostThreadMessage(thread_id, WM_QUIT, 0, 0);

        CloseHandle(thread);
        thread = NULL;
    }

    void win_stop_system_key_propagate(bool v)
    {
        stop_system_key_propagate = v;
    }

    // https://stackoverflow.com/questions/4023586/correct-way-to-find-out-if-a-service-is-running-as-the-system-user
} // end of extern "C"
