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

void flog(char const *fmt, ...)
{
    FILE *h = fopen("C:\\Windows\\temp\\test_rustdesk.log", "at");
    if (!h)
        return;
    va_list arg;
    va_start(arg, fmt);
    vfprintf(h, fmt, arg);
    va_end(arg);
    fclose(h);
}

static BOOL GetProcessUserName(DWORD processID, LPWSTR outUserName, DWORD inUserNameSize)
{
    BOOL ret = FALSE;
    HANDLE hProcess = NULL;
    HANDLE hToken = NULL;
    PTOKEN_USER tokenUser = NULL;
    wchar_t *userName = NULL;
    wchar_t *domainName = NULL;

    hProcess = OpenProcess(PROCESS_QUERY_INFORMATION, FALSE, processID);
    if (hProcess == NULL)
    {
        goto cleanup;
    }
    if (!OpenProcessToken(hProcess, TOKEN_QUERY, &hToken))
    {
        goto cleanup;
    }
    DWORD tokenInfoLength = 0;
    GetTokenInformation(hToken, TokenUser, NULL, 0, &tokenInfoLength);
    if (tokenInfoLength == 0)
    {
        goto cleanup;
    }
    tokenUser = (PTOKEN_USER)malloc(tokenInfoLength);
    if (tokenUser == NULL)
    {
        goto cleanup;
    }
    if (!GetTokenInformation(hToken, TokenUser, tokenUser, tokenInfoLength, &tokenInfoLength))
    {
        goto cleanup;
    }
    DWORD userSize = 0;
    DWORD domainSize = 0;
    SID_NAME_USE snu;
    LookupAccountSidW(NULL, tokenUser->User.Sid, NULL, &userSize, NULL, &domainSize, &snu);
    if (userSize == 0 || domainSize == 0)
    {
        goto cleanup;
    }
    userName = (wchar_t *)malloc((userSize + 1) * sizeof(wchar_t));
    if (userName == NULL)
    {
        goto cleanup;
    }
    domainName = (wchar_t *)malloc((domainSize + 1) * sizeof(wchar_t));
    if (domainName == NULL)
    {
        goto cleanup;
    }
    if (!LookupAccountSidW(NULL, tokenUser->User.Sid, userName, &userSize, domainName, &domainSize, &snu))
    {
        goto cleanup;
    }
    userName[userSize] = L'\0';
    domainName[domainSize] = L'\0';
    if (inUserNameSize <= userSize)
    {
        goto cleanup;
    }
    wcscpy(outUserName, userName);

    ret = TRUE;
cleanup:
    if (userName)
    {
        free(userName);
    }
    if (domainName)
    {
        free(domainName);
    }
    if (tokenUser != NULL)
    {
        free(tokenUser);
    }
    if (hToken != NULL)
    {
        CloseHandle(hToken);
    }
    if (hProcess != NULL)
    {
        CloseHandle(hProcess);
    }

    return ret;
}

// ultravnc has rdp support
// https://github.com/veyon/ultravnc/blob/master/winvnc/winvnc/service.cpp
// https://github.com/TigerVNC/tigervnc/blob/master/win/winvnc/VNCServerService.cxx
// https://blog.csdn.net/MA540213/article/details/84638264

DWORD GetLogonPid(DWORD dwSessionId, BOOL as_user)
{
    DWORD dwLogonPid = 0;
    HANDLE hSnap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (hSnap != INVALID_HANDLE_VALUE)
    {
        PROCESSENTRY32W procEntry;
        procEntry.dwSize = sizeof procEntry;

        if (Process32FirstW(hSnap, &procEntry))
            do
            {
                DWORD dwLogonSessionId = 0;
                if (_wcsicmp(procEntry.szExeFile, as_user ? L"explorer.exe" : L"winlogon.exe") == 0 &&
                    ProcessIdToSessionId(procEntry.th32ProcessID, &dwLogonSessionId) &&
                    dwLogonSessionId == dwSessionId)
                {
                    dwLogonPid = procEntry.th32ProcessID;
                    break;
                }
            } while (Process32NextW(hSnap, &procEntry));
        CloseHandle(hSnap);
    }
    return dwLogonPid;
}

static DWORD GetFallbackUserPid(DWORD dwSessionId)
{
    DWORD dwFallbackPid = 0;
    const wchar_t* fallbackUserProcs[] = {L"sihost.exe"};
    const int maxUsernameLen = 256;
    wchar_t sessionUsername[maxUsernameLen + 1] = {0};
    wchar_t processUsername[maxUsernameLen + 1] = {0};

    if (get_session_user_info(sessionUsername, maxUsernameLen, dwSessionId) == 0)
    {
        return 0;
    }
    HANDLE hSnap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (hSnap != INVALID_HANDLE_VALUE)
    {
        PROCESSENTRY32W procEntry;
        procEntry.dwSize = sizeof procEntry;

        if (Process32FirstW(hSnap, &procEntry))
            do
            {
                for (int i = 0; i < sizeof(fallbackUserProcs) / sizeof(fallbackUserProcs[0]); i++)
                {
                    DWORD dwProcessSessionId = 0;
                    if (_wcsicmp(procEntry.szExeFile, fallbackUserProcs[i]) == 0 &&
                        ProcessIdToSessionId(procEntry.th32ProcessID, &dwProcessSessionId) &&
                        dwProcessSessionId == dwSessionId)
                    {
                        memset(processUsername, 0, sizeof(processUsername));
                        if (GetProcessUserName(procEntry.th32ProcessID, processUsername, maxUsernameLen)) {
                            if (_wcsicmp(sessionUsername, processUsername) == 0)
                            {
                                dwFallbackPid = procEntry.th32ProcessID;
                                break;
                            }
                        }
                    }
                }
                if (dwFallbackPid != 0)
                {
                    break;
                }
            } while (Process32NextW(hSnap, &procEntry));
        CloseHandle(hSnap);
    }
    return dwFallbackPid;
}

// START the app as system
extern "C"
{
    // if should try WTSQueryUserToken?
    // https://stackoverflow.com/questions/7285666/example-code-a-service-calls-createprocessasuser-i-want-the-process-to-run-in
    BOOL GetSessionUserTokenWin(OUT LPHANDLE lphUserToken, DWORD dwSessionId, BOOL as_user, DWORD *pDwTokenPid)
    {
        BOOL bResult = FALSE;
        DWORD Id = GetLogonPid(dwSessionId, as_user);
        if (Id == 0)
        {
            Id = GetFallbackUserPid(dwSessionId);
        }
        if (pDwTokenPid)
            *pDwTokenPid = Id;
        if (HANDLE hProcess = OpenProcess(PROCESS_ALL_ACCESS, FALSE, Id))
        {
            bResult = OpenProcessToken(hProcess, TOKEN_ALL_ACCESS, lphUserToken);
            CloseHandle(hProcess);
        }
        return bResult;
    }

    bool is_windows_server()
    {
        return IsWindowsServer();
    }

    bool is_windows_10_or_greater()
    {
        return IsWindows10OrGreater();
    }

    HANDLE LaunchProcessWin(LPCWSTR cmd, DWORD dwSessionId, BOOL as_user, BOOL show, DWORD *pDwTokenPid)
    {
        HANDLE hProcess = NULL;
        HANDLE hToken = NULL;
        if (GetSessionUserTokenWin(&hToken, dwSessionId, as_user, pDwTokenPid))
        {
            STARTUPINFOW si;
            ZeroMemory(&si, sizeof si);
            si.cb = sizeof si;
            si.dwFlags = STARTF_USESHOWWINDOW;
            if (show)
            {
                si.lpDesktop = (LPWSTR)L"winsta0\\default";
                si.wShowWindow = SW_SHOW;
            }
            wchar_t buf[MAX_PATH];
            wcscpy_s(buf, MAX_PATH, cmd);
            PROCESS_INFORMATION pi;
            LPVOID lpEnvironment = NULL;
            DWORD dwCreationFlags = DETACHED_PROCESS;
            if (as_user)
            {

                CreateEnvironmentBlock(&lpEnvironment, // Environment block
                                       hToken,         // New token
                                       TRUE);          // Inheritance
            }
            if (lpEnvironment)
            {
                dwCreationFlags |= CREATE_UNICODE_ENVIRONMENT;
            }
            if (CreateProcessAsUserW(hToken, NULL, buf, NULL, NULL, FALSE, dwCreationFlags, lpEnvironment, NULL, &si, &pi))
            {
                CloseHandle(pi.hThread);
                hProcess = pi.hProcess;
            }
            CloseHandle(hToken);
            if (lpEnvironment)
                DestroyEnvironmentBlock(lpEnvironment);
        }
        return hProcess;
    }

} // end of extern "C"
