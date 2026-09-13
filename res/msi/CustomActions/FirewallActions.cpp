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

// No use for now, it can be refer as an example of ShellExecuteW.
void AddFirewallRuleCmdline(LPWSTR exeName, LPWSTR exeFile, LPCWSTR dir)
{
    HRESULT hr = S_OK;
    HINSTANCE hi = 0;
    WCHAR cmdline[1024] = { 0, };
    WCHAR rulename[500] = { 0, };

    StringCchPrintfW(rulename, sizeof(rulename) / sizeof(rulename[0]), L"%ls Service", exeName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make rulename: %ls", exeName);
        return;
    }

    StringCchPrintfW(cmdline, sizeof(cmdline) / sizeof(cmdline[0]), L"advfirewall firewall add rule name=\"%ls\" dir=%ls action=allow program=\"%ls\" enable=yes", rulename, dir, exeFile);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make cmdline: %ls", exeName);
        return;
    }

    hi = ShellExecuteW(NULL, L"open", L"netsh", cmdline, NULL, SW_HIDE);
    // https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to change firewall rule : %d, last error: %d", (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Firewall rule \"%ls\" (%ls) is added", rulename, dir);
    }
}

// No use for now, it can be refer as an example of ShellExecuteW.
void RemoveFirewallRuleCmdline(LPWSTR exeName)
{
    HRESULT hr = S_OK;
    HINSTANCE hi = 0;
    WCHAR cmdline[1024] = { 0, };
    WCHAR rulename[500] = { 0, };

    StringCchPrintfW(rulename, sizeof(rulename) / sizeof(rulename[0]), L"%ls Service", exeName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make rulename: %ls", exeName);
        return;
    }

    StringCchPrintfW(cmdline, sizeof(cmdline) / sizeof(cmdline[0]), L"advfirewall firewall delete rule name=\"%ls\"", rulename);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make cmdline: %ls", exeName);
        return;
    }

    hi = ShellExecuteW(NULL, L"open", L"netsh", cmdline, NULL, SW_HIDE);
    // https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to change firewall rule \"%ls\" : %d, last error: %d", rulename, (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Firewall rule \"%ls\" is removed", rulename);
    }
}

UINT __stdcall AddFirewallRules(
    __in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    int nResult = 0;
    LPWSTR exeFile = NULL;
    LPWSTR exeName = NULL;
    WCHAR exeNameNoExt[500] = { 0, };
    LPWSTR pwz = NULL;
    LPWSTR pwzData = NULL;
    size_t szNameLen = 0;

    hr = WcaInitialize(hInstall, "AddFirewallRules");
    ExitOnFailure(hr, "Failed to initialize");

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    pwz = pwzData;
    hr = WcaReadStringFromCaData(&pwz, &exeFile);
    ExitOnFailure(hr, "failed to read database key from custom action data: %ls", pwz);
    WcaLog(LOGMSG_STANDARD, "Try add firewall exceptions for file : %ls", exeFile);

    exeName = PathFindFileNameW(exeFile + 1);
    hr = StringCchPrintfW(exeNameNoExt, 500, exeName);
    ExitOnFailure(hr, "Failed to copy exe name: %ls", exeName);
    szNameLen = wcslen(exeNameNoExt);
    if (szNameLen >= 4 && wcscmp(exeNameNoExt + szNameLen - 4, L".exe") == 0) {
        exeNameNoExt[szNameLen - 4] = L'\0';
    }

    //if (exeFile[0] == L'1') {
    //    AddFirewallRuleCmdline(exeNameNoExt, exeFile, L"in");
    //    AddFirewallRuleCmdline(exeNameNoExt, exeFile, L"out");
    //}
    //else {
    //    RemoveFirewallRuleCmdline(exeNameNoExt);
    //}

    AddFirewallRule(exeFile[0] == L'1', exeNameNoExt, exeFile + 1);

LExit:
    if (pwzData) {
        ReleaseStr(pwzData);
    }

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}
