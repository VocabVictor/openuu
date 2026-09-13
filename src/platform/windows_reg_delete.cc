// https://github.com/rustdesk/rustdesk/discussions/6444#discussioncomment-9010062

#include <iostream>
#include <Windows.h>
#include <strsafe.h>

#include "windows_test_cert.h"

BOOL RegDelTestCertW(HKEY hKeyRoot, LPCWSTR lpSubKey)
{
	LONG lResult;
	HKEY hKey;
	DWORD dValueType;
	DWORD cchBufferSize = 0;
	BOOL bRes = FALSE;

	lResult = RegOpenKeyExW(hKeyRoot, lpSubKey, 0, KEY_READ, &hKey);
	if (lResult != ERROR_SUCCESS) {
		if (lResult == ERROR_FILE_NOT_FOUND) {
			return TRUE;
		}
		else {
			//printf("Error opening key.\n");
			return FALSE;
		}
	}

	do {
		lResult = RegQueryValueExW(hKey, L"Blob", NULL, &dValueType, NULL, &cchBufferSize);
		if (lResult == ERROR_SUCCESS) {
			if (dValueType == REG_BINARY) {
				LPSTR szBuffer = NULL;
				LONG readResult = 0;
				szBuffer = (LPSTR)malloc(cchBufferSize * sizeof(char));
				if (szBuffer == NULL) {
					bRes = FALSE;
					break;
				}

				lResult = RegQueryValueExW(hKey, L"Blob", NULL, &dValueType, (LPBYTE)szBuffer, &cchBufferSize);
				if (readResult == ERROR_SUCCESS) {
					if (IsCertWdkTestCert(szBuffer, cchBufferSize)) {
						free(szBuffer);
						lResult = RegDeleteKeyW(hKeyRoot, lpSubKey);
						if (lResult == ERROR_SUCCESS) {
							bRes = TRUE;
						}
						else {
							bRes = FALSE;
						}

						break;
					}
				}

				free(szBuffer);
			}
		}
	} while (FALSE);
	RegCloseKey(hKey);
	return bRes;
}

//*************************************************************
//
//  RegDelnodeRecurseW()
//
//  Purpose:    Deletes a registry key and all its subkeys / values.
//
//  Parameters: hKeyRoot    -   Root key
//              lpSubKey    -   SubKey to delete
//              bOneLevel   -   Delete lpSubKey and its first level subdirectory
//
//  Return:     TRUE if successful.
//              FALSE if an error occurs.
//
//  Note:       If bOneLevel is TRUE, only current key and its first level subkeys are deleted.
//              The first level subkeys are deleted only if they do not have subkeys.
//
//              If some subkeys have subkeys, but the previous empty subkeys are deleted.
//              It's ok for the certificates, because the empty subkeys are not used
//              and they can be created automatically.
//
//*************************************************************

BOOL RegDelnodeRecurseW(HKEY hKeyRoot, LPWSTR lpSubKey, BOOL bOneLevel)
{
	LPWSTR lpEnd;
	LONG lResult;
	DWORD dwSize;
	WCHAR szName[MAX_PATH];
	HKEY hKey;
	FILETIME ftWrite;

	// First, see if we can delete the key without having
	// to recurse.

	lResult = RegDeleteKeyW(hKeyRoot, lpSubKey);

	if (lResult == ERROR_SUCCESS)
		return TRUE;

	lResult = RegOpenKeyExW(hKeyRoot, lpSubKey, 0, KEY_READ, &hKey);

	if (lResult != ERROR_SUCCESS)
	{
		if (lResult == ERROR_FILE_NOT_FOUND) {
			//printf("Key not found.\n");
			return TRUE;
		}
		else {
			//printf("Error opening key.\n");
			return FALSE;
		}
	}

	// Check for an ending slash and add one if it is missing.

	lpEnd = lpSubKey + lstrlenW(lpSubKey);

	if (*(lpEnd - 1) != L'\\')
	{
		*lpEnd = L'\\';
		lpEnd++;
		*lpEnd = L'\0';
	}

	// Enumerate the keys

	dwSize = MAX_PATH;
	lResult = RegEnumKeyExW(hKey, 0, szName, &dwSize, NULL,
		NULL, NULL, &ftWrite);

	if (lResult == ERROR_SUCCESS)
	{
		do {

			*lpEnd = L'\0';
			StringCchCatW(lpSubKey, MAX_PATH * 2, szName);

			if (bOneLevel) {
				lResult = RegDeleteKeyW(hKeyRoot, lpSubKey);
				if (lResult != ERROR_SUCCESS) {
					return FALSE;
				}
			}
			else {
				if (!RegDelnodeRecurseW(hKeyRoot, lpSubKey, bOneLevel)) {
					break;
				}
			}

			dwSize = MAX_PATH;

			lResult = RegEnumKeyExW(hKey, 0, szName, &dwSize, NULL,
				NULL, NULL, &ftWrite);

		} while (lResult == ERROR_SUCCESS);
	}

	lpEnd--;
	*lpEnd = L'\0';

	RegCloseKey(hKey);

	// Try again to delete the key.

	lResult = RegDeleteKeyW(hKeyRoot, lpSubKey);

	if (lResult == ERROR_SUCCESS)
		return TRUE;

	return FALSE;
}

//*************************************************************
//
//  RegDelnodeW()
//
//  Purpose:    Deletes a registry key and all its subkeys / values.
//
//  Parameters: hKeyRoot    -   Root key
//              lpSubKey    -   SubKey to delete
//              bOneLevel   -   Delete lpSubKey and its first level subdirectory
//
//  Return:     TRUE if successful.
//              FALSE if an error occurs.
//
//*************************************************************

BOOL RegDelnodeW(HKEY hKeyRoot, LPCWSTR lpSubKey, BOOL bOneLevel)
{
	//return FALSE; // For Testing

	WCHAR szDelKey[MAX_PATH * 2];

	StringCchCopyW(szDelKey, MAX_PATH * 2, lpSubKey);
	return RegDelnodeRecurseW(hKeyRoot, szDelKey, bOneLevel);
}

//*************************************************************
//
//  DeleteRustDeskTestCertsW_SingleHive()
//
//  Purpose:    Deletes RustDesk Test certificates and wrong key stores
//
//  Parameters: RootKey     -   Root key
//              Prefix      -   SID if RootKey=HKEY_USERS
//
//  Return:     TRUE if successful.
//              FALSE if an error occurs.
//
//*************************************************************
