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
    DWORD get_current_session(BOOL include_rdp)
    {
        auto rdp_or_console = WTSGetActiveConsoleSessionId();
        if (!include_rdp)
            return rdp_or_console;
        PWTS_SESSION_INFOA pInfos;
        DWORD count;
        auto rdp = "rdp";
        auto nrdp = strlen(rdp);
        // https://github.com/rustdesk/rustdesk/discussions/937#discussioncomment-12373814 citrix session
        auto ica = "ica";
        auto nica = strlen(ica);
        if (WTSEnumerateSessionsA(WTS_CURRENT_SERVER_HANDLE, NULL, 1, &pInfos, &count))
        {
            for (DWORD i = 0; i < count; i++)
            {
                auto info = pInfos[i];
                if (info.State == WTSActive)
                {
                    if (info.pWinStationName == NULL)
                        continue;
                    if (!stricmp(info.pWinStationName, "console"))
                    {
                        auto id = info.SessionId;
                        WTSFreeMemory(pInfos);
                        return id;
                    }
                    if (!strnicmp(info.pWinStationName, rdp, nrdp) || !strnicmp(info.pWinStationName, ica, nica))
                    {
                        rdp_or_console = info.SessionId;
                    }
                }
            }
            WTSFreeMemory(pInfos);
        }
        return rdp_or_console;
    }

    BOOL is_session_locked(DWORD session_id)
    {
        if (session_id == 0xFFFFFFFF) {
            return FALSE;
        }
        PWTSINFOEXW pInfo = NULL;
        DWORD bytes = 0;
        BOOL locked = FALSE;
        if (WTSQuerySessionInformationW(
                WTS_CURRENT_SERVER_HANDLE,
                session_id,
                WTSSessionInfoEx,
                (LPWSTR *)&pInfo,
                &bytes)) {
            if (pInfo && pInfo->Level == 1) {
                locked = (pInfo->Data.WTSInfoExLevel1.SessionFlags == WTS_SESSIONSTATE_LOCK);
            }
            if (pInfo) {
                WTSFreeMemory(pInfo);
            }
        }
        return locked;
    }

    uint32_t get_active_user(PWSTR bufin, uint32_t nin, BOOL rdp)
    {
        uint32_t nout = 0;
        auto id = get_current_session(rdp);
        PWSTR buf = NULL;
        DWORD n = 0;
        if (WTSQuerySessionInformationW(WTS_CURRENT_SERVER_HANDLE, id, WTSUserName, &buf, &n))
        {
            if (buf)
            {
                nout = min(nin, n);
                memcpy(bufin, buf, nout);
                WTSFreeMemory(buf);
            }
        }
        return nout;
    }

    uint32_t get_session_user_info(PWSTR bufin, uint32_t nin, uint32_t id)
    {
        uint32_t nout = 0;
        PWSTR buf = NULL;
        DWORD n = 0;
        if (WTSQuerySessionInformationW(WTS_CURRENT_SERVER_HANDLE, id, WTSUserName, &buf, &n))
        {
            if (buf)
            {
                nout = min(nin, n);
                memcpy(bufin, buf, nout);
                WTSFreeMemory(buf);
            }
        }
        return nout;
    }

    void get_available_session_ids(PWSTR buf, uint32_t bufSize, BOOL include_rdp) {
        std::vector<std::wstring> sessionIds;
        PWTS_SESSION_INFOA pInfos = NULL;
        DWORD count;

        if (WTSEnumerateSessionsA(WTS_CURRENT_SERVER_HANDLE, 0, 1, &pInfos, &count)) {
            for (DWORD i = 0; i < count; i++) {
                auto info = pInfos[i];
                auto rdp = "rdp";
                auto nrdp = strlen(rdp);
                auto ica = "ica";
                auto nica = strlen(ica);
                if (info.State == WTSActive) {
                    if (info.pWinStationName == NULL)
                        continue;
                    if (info.SessionId == 65536 || info.SessionId == 655)
                        continue;

                    if (!stricmp(info.pWinStationName, "console")){
                        sessionIds.push_back(std::wstring(L"Console:") + std::to_wstring(info.SessionId));
                    }
                    else if (include_rdp && !strnicmp(info.pWinStationName, rdp, nrdp)) {
                        sessionIds.push_back(std::wstring(L"RDP:") + std::to_wstring(info.SessionId));
                    }
                    else if (include_rdp && !strnicmp(info.pWinStationName, ica, nica)) {
                        sessionIds.push_back(std::wstring(L"ICA:") + std::to_wstring(info.SessionId));
                    }
                }
            }
            WTSFreeMemory(pInfos);
        }

        std::wstring tmpStr;
        for (size_t i = 0; i < sessionIds.size(); i++) {
            if (i > 0) {
                tmpStr += L",";
            }
            tmpStr += sessionIds[i];
        }

        if (buf && !tmpStr.empty() && tmpStr.size() < bufSize) {
            wcsncpy_s(buf, bufSize, tmpStr.c_str(), tmpStr.size());
        }
    }

    BOOL is_local_system()
    {
        HANDLE hToken;
        UCHAR bTokenUser[sizeof(TOKEN_USER) + 8 + 4 * SID_MAX_SUB_AUTHORITIES];
        PTOKEN_USER pTokenUser = (PTOKEN_USER)bTokenUser;
        ULONG cbTokenUser;
        SID_IDENTIFIER_AUTHORITY siaNT = SECURITY_NT_AUTHORITY;
        PSID pSystemSid;
        BOOL bSystem;

        // open process token
        if (!OpenProcessToken(GetCurrentProcess(),
                              TOKEN_QUERY,
                              &hToken))
            return FALSE;

        // retrieve user SID
        if (!GetTokenInformation(hToken, TokenUser, pTokenUser,
                                 sizeof(bTokenUser), &cbTokenUser))
        {
            CloseHandle(hToken);
            return FALSE;
        }

        CloseHandle(hToken);

        // allocate LocalSystem well-known SID
        if (!AllocateAndInitializeSid(&siaNT, 1, SECURITY_LOCAL_SYSTEM_RID,
                                      0, 0, 0, 0, 0, 0, 0, &pSystemSid))
            return FALSE;

        // compare the user SID from the token with the LocalSystem SID
        bSystem = EqualSid(pTokenUser->User.Sid, pSystemSid);

        FreeSid(pSystemSid);

        return bSystem;
    }

    void alloc_console_and_redirect()
    {
        AllocConsole();
        freopen("CONOUT$", "w", stdout);
    }

    bool is_service_running_w(LPCWSTR serviceName)
    {
        SC_HANDLE hSCManager = OpenSCManagerW(NULL, NULL, SC_MANAGER_CONNECT);
        if (hSCManager == NULL) {
            return false;
        }

        SC_HANDLE hService = OpenServiceW(hSCManager, serviceName, SERVICE_QUERY_STATUS);
        if (hService == NULL) {
            CloseServiceHandle(hSCManager);
            return false;
        }

        SERVICE_STATUS_PROCESS serviceStatus;
        DWORD bytesNeeded;
        if (!QueryServiceStatusEx(hService, SC_STATUS_PROCESS_INFO, reinterpret_cast<LPBYTE>(&serviceStatus), sizeof(serviceStatus), &bytesNeeded)) {
            CloseServiceHandle(hService);
            CloseServiceHandle(hSCManager);
            return false;
        }

        bool isRunning = (serviceStatus.dwCurrentState == SERVICE_RUNNING);

        CloseServiceHandle(hService);
        CloseServiceHandle(hSCManager);

        return isRunning;
    }
} // end of extern "C"
