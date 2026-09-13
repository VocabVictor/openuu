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

UINT __stdcall SetPropertyIsServiceRunning(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    wchar_t szAppName[500] = { 0 };
    DWORD cchAppName = sizeof(szAppName) / sizeof(szAppName[0]);
    wchar_t szPropertyName[500] = { 0 };
    DWORD cchPropertyName = sizeof(szPropertyName) / sizeof(szPropertyName[0]);
    bool isRunning = false;

    hr = WcaInitialize(hInstall, "SetPropertyIsServiceRunning");
    ExitOnFailure(hr, "Failed to initialize");

    MsiGetPropertyW(hInstall, L"AppName", szAppName, &cchAppName);
    WcaLog(LOGMSG_STANDARD, "Try query service of : \"%ls\"", szAppName);

    MsiGetPropertyW(hInstall, L"PropertyName", szPropertyName, &cchPropertyName);
    WcaLog(LOGMSG_STANDARD, "Try set is service running, property name : \"%ls\"", szPropertyName);

    isRunning = IsServiceRunningW(szAppName);
    MsiSetPropertyW(hInstall, szPropertyName, isRunning ? L"'N'" : L"'Y'");

LExit:
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

// The in-app installer seeds the SYSTEM-side config by running a temporary
// service with `--import-config <user config>` before creating the real one;
// without it the service starts with a fresh ID and no ID/API server or key.
void TryImportConfigByTempService(LPCWSTR svcName, LPCWSTR svcBinary, LPCWSTR configPath)
{
    HRESULT hr = S_OK;
    wchar_t szTempName[500] = { 0 };
    wchar_t szExe[500] = { 0 };
    wchar_t szBin[1200] = { 0 };
    SERVICE_STATUS_PROCESS svcStatus;
    LPCWSTR exeEnd = NULL;
    size_t exeLen = 0;

    if (configPath == NULL || configPath[0] == L'\0') {
        return;
    }
    if (!PathFileExistsW(configPath)) {
        WcaLog(LOGMSG_STANDARD, "No user config to import: %ls", configPath);
        return;
    }
    // svcBinary is `"<exe>" --service`; reuse the quoted exe path.
    if (svcBinary[0] != L'"' || (exeEnd = wcschr(svcBinary + 1, L'"')) == NULL) {
        WcaLog(LOGMSG_STANDARD, "Cannot find exe in service binary: %ls", svcBinary);
        return;
    }
    exeLen = exeEnd - (svcBinary + 1);
    if (exeLen >= sizeof(szExe) / sizeof(szExe[0])) {
        WcaLog(LOGMSG_STANDARD, "Service exe path too long: %ls", svcBinary);
        return;
    }
    wcsncpy_s(szExe, svcBinary + 1, exeLen);
    hr = StringCchPrintfW(szTempName, sizeof(szTempName) / sizeof(szTempName[0]), L"%lsConfigImport", svcName);
    if (FAILED(hr)) {
        return;
    }
    hr = StringCchPrintfW(szBin, sizeof(szBin) / sizeof(szBin[0]), L"\"%ls\" --import-config \"%ls\"", szExe, configPath);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to compose import-config command");
        return;
    }
    WcaLog(LOGMSG_STANDARD, "Import user config: %ls", szBin);
    // A stale temp service from an interrupted install would block CreateService.
    MyDeleteServiceW(szTempName);
    if (!MyCreateServiceW(szTempName, szTempName, szBin)) {
        WcaLog(LOGMSG_STANDARD, "Failed to create import service: %ls", szTempName);
        return;
    }
    // `--import-config` exits without reporting to the SCM, so the start
    // request fails once the process has finished; that is the expected outcome.
    MyStartServiceW(szTempName);
    for (int k = 0; k < 20; ++k) {
        if (!QueryServiceStatusExW(szTempName, &svcStatus) || svcStatus.dwCurrentState == SERVICE_STOPPED) {
            break;
        }
        Sleep(500);
    }
    MyDeleteServiceW(szTempName);
}

UINT __stdcall CreateStartService(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    LPWSTR svcParams = NULL;
    LPWSTR pwz = NULL;
    LPWSTR pwzData = NULL;
    LPWSTR svcName = NULL;
    LPWSTR svcBinary = NULL;
    LPWSTR configPath = NULL;
    wchar_t szSvcDisplayName[500] = { 0 };
    DWORD cchSvcDisplayName = sizeof(szSvcDisplayName) / sizeof(szSvcDisplayName[0]);

    hr = WcaInitialize(hInstall, "CreateStartService");
    ExitOnFailure(hr, "Failed to initialize");

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    pwz = pwzData;
    hr = WcaReadStringFromCaData(&pwz, &svcParams);
    ExitOnFailure(hr, "failed to read database key from custom action data: %ls", pwz);

    WcaLog(LOGMSG_STANDARD, "Try create start service : %ls", svcParams);

    svcName = svcParams;
    svcBinary = wcschr(svcParams, L';');
    if (svcBinary == NULL) {
        WcaLog(LOGMSG_STANDARD, "Failed to find binary : %ls", svcParams);
        goto LExit;
    }
    svcBinary[0] = L'\0';
    svcBinary += 1;
    // `|` cannot occur in a Windows path, unlike `;`.
    configPath = wcschr(svcBinary, L'|');
    if (configPath != NULL) {
        configPath[0] = L'\0';
        configPath += 1;
    }

    hr = StringCchPrintfW(szSvcDisplayName, cchSvcDisplayName, L"%ls Service", svcName);
    ExitOnFailure(hr, "Failed to compose a resource identifier string");
    TryImportConfigByTempService(svcName, svcBinary, configPath);
    if (MyCreateServiceW(svcName, szSvcDisplayName, svcBinary)) {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is created.", svcName);
        if (MyStartServiceW(svcName)) {
            WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is started.", svcName);
        }
        else {
            WcaLog(LOGMSG_STANDARD, "Failed to start service: \"%ls\"", svcName);
        }
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Failed to create service: \"%ls\"", svcName);
    }

    if (IsServiceRunningW(svcName)) {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is running.", svcName);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is not running, try create and start service by shell", svcName);
        TryCreateStartServiceByShell(svcName, svcBinary, szSvcDisplayName);
    }

LExit:
    if (pwzData) {
        ReleaseStr(pwzData);
    }

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

UINT __stdcall TryStopDeleteService(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    int nResult = 0;
    LPWSTR svcName = NULL;
    LPWSTR pwz = NULL;
    LPWSTR pwzData = NULL;
    wchar_t szExeFile[500] = { 0 };
    DWORD cchExeFile = sizeof(szExeFile) / sizeof(szExeFile[0]);
    SERVICE_STATUS_PROCESS svcStatus;
    DWORD lastErrorCode = 0;

    hr = WcaInitialize(hInstall, "TryStopDeleteService");
    ExitOnFailure(hr, "Failed to initialize");

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    pwz = pwzData;
    hr = WcaReadStringFromCaData(&pwz, &svcName);
    ExitOnFailure(hr, "failed to read database key from custom action data: %ls", pwz);
    WcaLog(LOGMSG_STANDARD, "Try stop and delete service : %ls", svcName);

    if (MyStopServiceW(svcName)) {
        for (int i = 0; i < 10; i++) {
            if (IsServiceRunningW(svcName)) {
                Sleep(100);
            }
            else {
                break;
            }
        }
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Failed to stop service: \"%ls\", error: 0x%02X.", svcName, GetLastError());
    }

    if (IsServiceRunningW(svcName)) {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is not stopped after 1000 ms.", svcName);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is stopped.", svcName);
    }

    if (MyDeleteServiceW(svcName)) {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" deletion is completed without errors.", svcName);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Failed to delete service: \"%ls\", error: 0x%02X.", svcName, GetLastError());
    }

    if (QueryServiceStatusExW(svcName, &svcStatus)) {
        WcaLog(LOGMSG_STANDARD, "Failed to delete service: \"%ls\", current status: %d.", svcName, svcStatus.dwCurrentState);
        TryStopDeleteServiceByShell(svcName);
    }
    else {
        lastErrorCode = GetLastError();
        if (lastErrorCode == ERROR_SERVICE_DOES_NOT_EXIST) {
            WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is deleted.", svcName);
        }
        else {
            WcaLog(LOGMSG_STANDARD, "Failed to query service status: \"%ls\", error: 0x%02X.", svcName, lastErrorCode);
            TryStopDeleteServiceByShell(svcName);
        }
    }

    // It's really strange that we need sleep here.
    // But the upgrading may be stuck at "copying new files" because the file is in using.
    // Steps to reproduce: Install -> stop service in tray --> start service -> upgrade
    // Sleep(300);

    // Or we can terminate the process
    hr = StringCchPrintfW(szExeFile, cchExeFile, L"%ls.exe", svcName);
    ExitOnFailure(hr, "Failed to compose a resource identifier string");
    TerminateProcessesByNameW(szExeFile, L"--not-in-use");

LExit:
    if (pwzData) {
        ReleaseStr(pwzData);
    }

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}
