// CustomAction.cpp : Defines the entry point for the custom action.
#include "pch.h"
#include <stdlib.h>
#include <strutil.h>
#include <shellapi.h>
#include <tlhelp32.h>
#include <winternl.h>
#include <netfw.h>
#include <shlwapi.h>

#include "./Common.h"

#pragma comment(lib, "Shlwapi.lib")

void TryCreateStartServiceByShell(LPWSTR svcName, LPWSTR svcBinary, LPWSTR szSvcDisplayName)
{
    HRESULT hr = S_OK;
    HINSTANCE hi = 0;
    wchar_t szNewBin[500] = { 0 };
    DWORD cchNewBin = sizeof(szNewBin) / sizeof(szNewBin[0]);
    wchar_t szCmd[800] = { 0 };
    DWORD cchCmd = sizeof(szCmd) / sizeof(szCmd[0]);
    SERVICE_STATUS_PROCESS svcStatus;
    DWORD lastErrorCode = 0;
    int i = 0;
    int j = 0;

    WcaLog(LOGMSG_STANDARD, "TryCreateStartServiceByShell, service: %ls", svcName);

    TryStopDeleteServiceByShell(svcName);
    // Do not check the result here

    i = 0;
    j = 0;
    // svcBinary is a string with double quotes, we need to escape it for shell arguments.
    // It is original used for `CreateServiceW`.
    // eg. "C:\Program Files\MyApp\MyApp.exe" --service -> \"C:\Program Files\MyApp\MyApp.exe\" --service
    while (true) {
        if (svcBinary[j] == L'"') {
            szNewBin[i] = L'\\';
            i += 1;
            if (i >= cchNewBin) {
                WcaLog(LOGMSG_STANDARD, "Failed to copy bin for service: %ls, buffer is not enough", svcName);
                return;
            }
            szNewBin[i] = L'"';
        }
        else {
            szNewBin[i] = svcBinary[j];
        }
        if (svcBinary[j] == L'\0') {
            break;
        }
        i += 1;
        j += 1;
        if (i >= cchNewBin) {
            WcaLog(LOGMSG_STANDARD, "Failed to copy bin for service: %ls, buffer is not enough", svcName);
            return;
        }
    }

    hr = StringCchPrintfW(szCmd, cchCmd, L"create %ls binpath= \"%ls\" start= auto DisplayName= \"%ls\"", svcName, szNewBin, szSvcDisplayName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make command: %ls", svcName);
        return;
    }
    hi = ShellExecuteW(NULL, L"open", L"sc", szCmd, NULL, SW_HIDE);
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to create service with shell : %d, last error: 0x%02X.", (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is created with shell.", svcName);
    }

    // Query and log if the service is running.
    for (int k = 0; k < 10; ++k) {
        if (!QueryServiceStatusExW(svcName, &svcStatus)) {
            lastErrorCode = GetLastError();
            if (lastErrorCode == ERROR_SERVICE_DOES_NOT_EXIST) {
                if (k == 29) {
                    WcaLog(LOGMSG_STANDARD, "Failed to query service status: \"%ls\", service is not found.", svcName);
                    return;
                }
                else {
                    Sleep(100);
                    continue;
                }
            }
            // Break if the service exists.
            WcaLog(LOGMSG_STANDARD, "Failed to query service status: \"%ls\", error: 0x%02X.", svcName, lastErrorCode);
            break;
        }
        else {
            if (svcStatus.dwCurrentState == SERVICE_RUNNING) {
                WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is running.", svcName);
                return;
            }
            WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is not running.", svcName);
            break;
        }
    }

    hr = StringCchPrintfW(szCmd, cchCmd, L"/c sc start %ls", svcName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make command: %ls", svcName);
        return;
    }
    hi = ShellExecuteW(NULL, L"open", L"cmd.exe", szCmd, NULL, SW_HIDE);
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to start service with shell : %d, last error: 0x%02X.", (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is started with shell.", svcName);
    }
}

void TryStopDeleteServiceByShell(LPWSTR svcName)
{
    HRESULT hr = S_OK;
    HINSTANCE hi = 0;
    wchar_t szCmd[800] = { 0 };
    DWORD cchCmd = sizeof(szCmd) / sizeof(szCmd[0]);
    SERVICE_STATUS_PROCESS svcStatus;
    DWORD lastErrorCode = 0;

    WcaLog(LOGMSG_STANDARD, "TryStopDeleteServiceByShell, service: %ls", svcName);

    hr = StringCchPrintfW(szCmd, cchCmd, L"/c sc stop %ls", svcName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make command: %ls", svcName);
        return;
    }
    hi = ShellExecuteW(NULL, L"open", L"cmd.exe", szCmd, NULL, SW_HIDE);

    // Query and log if the service is stopped or deleted.
    for (int k = 0; k < 10; ++k) {
        if (!IsServiceRunningW(svcName)) {
            break;
        }
        Sleep(100);
    }
    if (!QueryServiceStatusExW(svcName, &svcStatus)) {
        if (GetLastError() == ERROR_SERVICE_DOES_NOT_EXIST) {
            WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is already deleted.", svcName);
            return;
        }
        WcaLog(LOGMSG_STANDARD, "Failed to query service status: \"%ls\" with shell, error: 0x%02X.", svcName, lastErrorCode);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Status of service: \"%ls\" with shell, current status: %d.", svcName, svcStatus.dwCurrentState);
    }

    hr = StringCchPrintfW(szCmd, cchCmd, L"/c sc delete %ls", svcName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make command: %ls", svcName);
        return;
    }
    hi = ShellExecuteW(NULL, L"open", L"cmd.exe", szCmd, NULL, SW_HIDE);
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to delete service with shell : %d, last error: 0x%02X.", (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" deletion is completed without errors with shell,", svcName);
    }

    // Query and log the status of the service after deletion.
    for (int k = 0; k < 10; ++k) {
        if (!QueryServiceStatusExW(svcName, &svcStatus)) {
            if (GetLastError() == ERROR_SERVICE_DOES_NOT_EXIST) {
                WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is deleted with shell.", svcName);
                return;
            }
        }
        Sleep(100);
    }
    if (!QueryServiceStatusExW(svcName, &svcStatus)) {
        lastErrorCode = GetLastError();
        if (lastErrorCode == ERROR_SERVICE_DOES_NOT_EXIST) {
            WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is deleted with shell.", svcName);
            return;
        }
        WcaLog(LOGMSG_STANDARD, "Failed to query service status: \"%ls\" with shell, error: 0x%02X.", svcName, lastErrorCode);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Failed to delete service: \"%ls\" with shell, current status: %d.", svcName, svcStatus.dwCurrentState);
    }
}
