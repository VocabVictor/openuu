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

// Helper function to safely delete a file using handle-based deletion.
// Directories are refused after opening the handle.
BOOL SafeDeleteItem(LPCWSTR fullPath)
{
    // Open the file/directory with delete and attribute-read access plus FILE_FLAG_OPEN_REPARSE_POINT
    // to prevent following symlinks.
    // Use shared access to allow deletion even when other processes have the file open.
    DWORD flags = FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT;
    HANDLE hFile = CreateFileW(
        fullPath,
        DELETE | FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,  // Allow shared access
        NULL,
        OPEN_EXISTING,
        flags,
        NULL
    );

    if (hFile == INVALID_HANDLE_VALUE)
    {
        WcaLog(LOGMSG_STANDARD, "SafeDeleteItem: Failed to open '%ls'. Error: %lu", fullPath, GetLastError());
        return FALSE;
    }

    BY_HANDLE_FILE_INFORMATION fileInfo;
    if (FALSE == GetFileInformationByHandle(hFile, &fileInfo))
    {
        WcaLog(LOGMSG_STANDARD, "SafeDeleteItem: Failed to inspect '%ls'. Error: %lu", fullPath, GetLastError());
        CloseHandle(hFile);
        return FALSE;
    }

    if (fileInfo.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)
    {
        WcaLog(LOGMSG_STANDARD, "SafeDeleteItem: Refusing to delete directory '%ls'.", fullPath);
        CloseHandle(hFile);
        return FALSE;
    }

    // Use SetFileInformationByHandle to mark for deletion.
    // The file will be deleted when the handle is closed.
    FILE_DISPOSITION_INFO dispInfo;
    dispInfo.DeleteFile = TRUE;

    BOOL result = SetFileInformationByHandle(
        hFile,
        FileDispositionInfo,
        &dispInfo,
        sizeof(dispInfo)
    );

    if (!result)
    {
        DWORD error = GetLastError();
        WcaLog(LOGMSG_STANDARD, "SafeDeleteItem: Failed to mark '%ls' for deletion. Error: %lu", fullPath, error);
    }

    CloseHandle(hFile);
    return result;
}

BOOL PathEndsWithSlash(LPCWSTR path)
{
    size_t length = 0;
    HRESULT hr = StringCchLengthW(path, MAX_PATH, &length);
    if (FAILED(hr) || length == 0)
    {
        return FALSE;
    }

    WCHAR last = path[length - 1];
    return last == L'\\' || last == L'/';
}

void ClearReadOnlyAttribute(LPCWSTR fullPath, DWORD attributes)
{
    if (!(attributes & FILE_ATTRIBUTE_READONLY))
    {
        return;
    }

    DWORD writableAttributes = attributes & ~FILE_ATTRIBUTE_READONLY;
    if (writableAttributes == 0)
    {
        writableAttributes = FILE_ATTRIBUTE_NORMAL;
    }

    if (SetFileAttributesW(fullPath, writableAttributes))
    {
        WcaLog(LOGMSG_STANDARD, "Runtime cleanup cleared read-only attribute for '%ls'.", fullPath);
        return;
    }

    WcaLog(LOGMSG_STANDARD, "Runtime cleanup failed to clear read-only attribute for '%ls'. Error: %lu", fullPath, GetLastError());
}

BOOL DeleteRuntimeGeneratedFile(LPCWSTR installFolder, LPCWSTR fileName)
{
    WCHAR fullPath[MAX_PATH];
    LPCWSTR separator = PathEndsWithSlash(installFolder) ? L"" : L"\\";
    HRESULT hr = StringCchPrintfW(fullPath, MAX_PATH, L"%s%s%s", installFolder, separator, fileName);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Runtime cleanup path is too long for '%ls'.", fileName);
        return FALSE;
    }

    DWORD attributes = GetFileAttributesW(fullPath);
    if (attributes == INVALID_FILE_ATTRIBUTES)
    {
        DWORD error = GetLastError();
        if (error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND)
        {
            return TRUE;
        }

        WcaLog(LOGMSG_STANDARD, "Runtime cleanup cannot stat '%ls'. Error: %lu", fullPath, error);
        return FALSE;
    }

    if (attributes & FILE_ATTRIBUTE_DIRECTORY)
    {
        WcaLog(LOGMSG_STANDARD, "Runtime cleanup skipped directory '%ls'.", fullPath);
        return FALSE;
    }

    ClearReadOnlyAttribute(fullPath, attributes);
    WcaLog(LOGMSG_STANDARD, "Runtime cleanup deleting '%ls'.", fullPath);
    return SafeDeleteItem(fullPath);
}

// See `Package.wxs` for the sequence of this custom action.
//
// Upgrade/uninstall sequence:
//   1. InstallInitialize
//   2. RemoveExistingProducts
//      ├─ TerminateProcesses
//      ├─ TryStopDeleteService
//      ├─ RemoveRuntimeGeneratedFiles - <-- Here
//      └─ RemoveFiles
//   3. InstallValidate
//   4. InstallFiles
//   5. InstallExecute
//   6. InstallFinalize
UINT __stdcall RemoveRuntimeGeneratedFiles(
    __in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    LPWSTR installFolder = NULL;
    LPWSTR pwz = NULL;
    LPWSTR pwzData = NULL;

    hr = WcaInitialize(hInstall, "RemoveRuntimeGeneratedFiles");
    ExitOnFailure(hr, "Failed to initialize");

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    pwz = pwzData;
    hr = WcaReadStringFromCaData(&pwz, &installFolder);
    ExitOnFailure(hr, "failed to read install folder from custom action data: %ls", pwz);

    if (installFolder == NULL || installFolder[0] == L'\0') {
        WcaLog(LOGMSG_STANDARD, "Install folder path is empty, skipping runtime cleanup.");
        goto LExit;
    }

    if (PathIsRootW(installFolder)) {
        WcaLog(LOGMSG_STANDARD, "Refusing runtime cleanup in root folder '%ls'.", installFolder);
        goto LExit;
    }

    WcaLog(LOGMSG_STANDARD, "Removing runtime-generated files from install folder: %ls", installFolder);
    DeleteRuntimeGeneratedFile(installFolder, L"RuntimeBroker_rustdesk.exe");

LExit:
    ReleaseStr(pwzData);

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}
