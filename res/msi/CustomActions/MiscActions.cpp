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

UINT __stdcall TryDeleteStartupShortcut(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    wchar_t szShortcut[500] = { 0 };
    DWORD cchShortcut = sizeof(szShortcut) / sizeof(szShortcut[0]);
    wchar_t szStartupDir[500] = { 0 };
    DWORD cchStartupDir = sizeof(szStartupDir) / sizeof(szStartupDir[0]);
    WCHAR pwszTemp[1024] = L"";

    hr = WcaInitialize(hInstall, "DeleteStartupShortcut");
    ExitOnFailure(hr, "Failed to initialize");

    MsiGetPropertyW(hInstall, L"StartupFolder", szStartupDir, &cchStartupDir);

    MsiGetPropertyW(hInstall, L"ShortcutName", szShortcut, &cchShortcut);
    WcaLog(LOGMSG_STANDARD, "Try delete startup shortcut of : \"%ls\"", szShortcut);

    hr = StringCchPrintfW(pwszTemp, 1024, L"%ls%ls.lnk", szStartupDir, szShortcut);
    ExitOnFailure(hr, "Failed to compose a resource identifier string");

    if (DeleteFileW(pwszTemp)) {
        WcaLog(LOGMSG_STANDARD, "Failed to delete startup shortcut of : \"%ls\"", pwszTemp);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Startup shortcut is deleted : \"%ls\"", pwszTemp);
    }

LExit:
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

UINT __stdcall SetPropertyFromConfig(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    wchar_t szConfigFile[1024] = { 0 };
    DWORD cchConfigFile = sizeof(szConfigFile) / sizeof(szConfigFile[0]);
    wchar_t szConfigKey[500] = { 0 };
    DWORD cchConfigKey = sizeof(szConfigKey) / sizeof(szConfigKey[0]);
    wchar_t szPropertyName[500] = { 0 };
    DWORD cchPropertyName = sizeof(szPropertyName) / sizeof(szPropertyName[0]);
    std::wstring configValue;

    hr = WcaInitialize(hInstall, "SetPropertyFromConfig");
    ExitOnFailure(hr, "Failed to initialize");

    MsiGetPropertyW(hInstall, L"ConfigFile", szConfigFile, &cchConfigFile);
    WcaLog(LOGMSG_STANDARD, "Try read config file of : \"%ls\"", szConfigFile);

    MsiGetPropertyW(hInstall, L"ConfigKey", szConfigKey, &cchConfigKey);
    WcaLog(LOGMSG_STANDARD, "Try read configuration, config key : \"%ls\"", szConfigKey);

    MsiGetPropertyW(hInstall, L"PropertyName", szPropertyName, &cchPropertyName);
    WcaLog(LOGMSG_STANDARD, "Try read configuration, property name : \"%ls\"", szPropertyName);

    configValue = ReadConfig(szConfigFile, szConfigKey);
    MsiSetPropertyW(hInstall, szPropertyName, configValue.c_str());

LExit:
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

UINT __stdcall AddRegSoftwareSASGeneration(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

     LSTATUS result = 0;
     HKEY hKey;
     LPCWSTR subKey = L"Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System";
     LPCWSTR valueName = L"SoftwareSASGeneration";
     DWORD valueType = REG_DWORD;
     DWORD valueData = 1;
     DWORD valueDataSize = sizeof(DWORD);

    HINSTANCE hi = 0;

    hr = WcaInitialize(hInstall, "AddRegSoftwareSASGeneration");
    ExitOnFailure(hr, "Failed to initialize");

    hi = ShellExecuteW(NULL, L"open", L"reg", L" add HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System /f /v SoftwareSASGeneration /t REG_DWORD /d 1", NULL, SW_HIDE);
    // https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to add registry name \"%ls\", %d, %d", valueName, (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Registry name \"%ls\" is added", valueName);
    }

    // Why RegSetValueExW always return 998?
    //
    result = RegCreateKeyExW(HKEY_LOCAL_MACHINE, subKey, 0, NULL, REG_OPTION_NON_VOLATILE, KEY_WRITE, NULL, &hKey, NULL);
    if (result != ERROR_SUCCESS) {
        WcaLog(LOGMSG_STANDARD, "Failed to create or open registry key: %d", result);
        goto LExit;
    }

    result = RegSetValueExW(hKey, valueName, 0, valueType, reinterpret_cast<const BYTE*>(valueData), valueDataSize);
    if (result != ERROR_SUCCESS) {
        WcaLog(LOGMSG_STANDARD, "Failed to set registry value: %d", result);
        RegCloseKey(hKey);
        goto LExit;
    }

    WcaLog(LOGMSG_STANDARD, "Registry value has been successfully set.");
    RegCloseKey(hKey);

LExit:
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

UINT __stdcall RemoveAmyuniIdd(
    __in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    int nResult = 0;
    LPWSTR installFolder = NULL;
    LPWSTR pwz = NULL;
    LPWSTR pwzData = NULL;

    WCHAR workDir[1024] = L"";
    DWORD fileAttributes = 0;
    HINSTANCE hi = 0;

    SYSTEM_INFO si;
    LPCWSTR exe = L"deviceinstaller64.exe";
    WCHAR exePath[1024] = L"";

    BOOL rebootRequired = FALSE;

    hr = WcaInitialize(hInstall, "RemoveAmyuniIdd");
    ExitOnFailure(hr, "Failed to initialize");

    UninstallDriver(L"usbmmidd", rebootRequired);

    // Only for x86 app on x64
    GetNativeSystemInfo(&si);
    if (si.wProcessorArchitecture != PROCESSOR_ARCHITECTURE_AMD64) {
        goto LExit;
    }

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    pwz = pwzData;
    hr = WcaReadStringFromCaData(&pwz, &installFolder);
    ExitOnFailure(hr, "failed to read database key from custom action data: %ls", pwz);

    hr = StringCchPrintfW(workDir, 1024, L"%lsusbmmidd_v2", installFolder);
    ExitOnFailure(hr, "Failed to compose a resource identifier string");
    fileAttributes = GetFileAttributesW(workDir);
    if (fileAttributes == INVALID_FILE_ATTRIBUTES) {
        WcaLog(LOGMSG_STANDARD, "Amyuni idd dir \"%ls\" is not found, %d", workDir, fileAttributes);
        goto LExit;
    }

    hr = StringCchPrintfW(exePath, 1024, L"%ls\\%ls", workDir, exe);
    ExitOnFailure(hr, "Failed to compose a resource identifier string");
    fileAttributes = GetFileAttributesW(exePath);
    if (fileAttributes == INVALID_FILE_ATTRIBUTES) {
        goto LExit;
    }

    WcaLog(LOGMSG_STANDARD, "Remove amyuni idd %ls in %ls", exe, workDir);
    hi = ShellExecuteW(NULL, L"open", exe, L"remove usbmmidd", workDir, SW_HIDE);
    // https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to remove amyuni idd : %d, last error: %d", (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Amyuni idd is removed");
    }

LExit:
    if (pwzData) {
        ReleaseStr(pwzData);
    }

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

// OPENUU_CONFIG=<path>: copy the provisioning file next to the exe so every start
// imports it (see src/common/provision.rs). CustomActionData is "<src>|<dst>".
UINT __stdcall CopyProvisionConfig(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;
    LPWSTR pwzData = NULL;
    LPWSTR pwz = NULL;
    LPWSTR src = NULL;
    LPWSTR dst = NULL;

    hr = WcaInitialize(hInstall, "CopyProvisionConfig");
    ExitOnFailure(hr, "Failed to initialize");

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    pwz = pwzData;
    hr = WcaReadStringFromCaData(&pwz, &src);
    ExitOnFailure(hr, "failed to read custom action data: %ls", pwz);

    dst = wcschr(src, L'|');
    if (dst == NULL || src[0] == L'\0') {
        WcaLog(LOGMSG_STANDARD, "CopyProvisionConfig: no source or destination in \"%ls\"", src);
        goto LExit;
    }
    dst[0] = L'\0';
    dst += 1;
    if (!PathFileExistsW(src)) {
        WcaLog(LOGMSG_STANDARD, "CopyProvisionConfig: source does not exist: %ls", src);
        goto LExit;
    }
    if (CopyFileW(src, dst, FALSE)) {
        WcaLog(LOGMSG_STANDARD, "CopyProvisionConfig: copied %ls to %ls", src, dst);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "CopyProvisionConfig: failed to copy %ls to %ls, error: %d", src, dst, GetLastError());
    }

LExit:
    if (pwzData) {
        ReleaseStr(pwzData);
    }
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}
