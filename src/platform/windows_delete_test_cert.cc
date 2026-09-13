// https://github.com/rustdesk/rustdesk/discussions/6444#discussioncomment-9010062

#include <iostream>
#include <Windows.h>
#include <strsafe.h>

#include "windows_test_cert.h"

BOOL DeleteRustDeskTestCertsW_SingleHive(HKEY RootKey, LPWSTR Prefix = NULL) {
	// WDKTestCert to be removed from all stores
	LPCWSTR lpCertFingerPrint = L"D1DBB672D5A500B9809689CAEA1CE49E799767F0";

	// Wrong key stores to be removed completely
	LPCSTR RootName = "ROOT";
	LPWSTR SubKeyPrefix = (LPWSTR)RootName; // sic! Convert of ANSI to UTF-16

	LPWSTR lpSystemCertificatesPath = (LPWSTR)malloc(512 * sizeof(WCHAR));
	if (lpSystemCertificatesPath == 0) return FALSE;
	if (Prefix == NULL) {
		wsprintfW(lpSystemCertificatesPath, L"Software\\Microsoft\\SystemCertificates");
	}
	else {
		wsprintfW(lpSystemCertificatesPath, L"%s\\Software\\Microsoft\\SystemCertificates", Prefix);
	}

	HKEY hRegSystemCertificates;
	LONG res = RegOpenKeyExW(RootKey, lpSystemCertificatesPath, NULL, KEY_ALL_ACCESS, &hRegSystemCertificates);
	if (res != ERROR_SUCCESS)
		return FALSE;

	for (DWORD Index = 0; ; Index++) {
		LPWSTR SubKeyName = (LPWSTR)malloc(255 * sizeof(WCHAR));
		if (SubKeyName == 0) break;
		DWORD cName = 255;
		LONG res = RegEnumKeyExW(hRegSystemCertificates, Index, SubKeyName, &cName, NULL, NULL, NULL, NULL);
		if ((res != ERROR_SUCCESS) || (SubKeyName == NULL))
			break;

		// Remove test certificate
		LPWSTR Complete = (LPWSTR)malloc(512 * sizeof(WCHAR));
		if (Complete == 0) break;
		wsprintfW(Complete, L"%s\\%s\\Certificates\\%s", lpSystemCertificatesPath, SubKeyName, lpCertFingerPrint);
		// std::wcout << "Try delete from: " << SubKeyName << std::endl;
		RegDelTestCertW(RootKey, Complete);
		free(Complete);

		// "佒呏..." key begins with "ROOT" encoded as UTF-16
		if ((SubKeyName[0] == SubKeyPrefix[0]) && (SubKeyName[1] == SubKeyPrefix[1])) {
			// Remove wrong empty key store
			{
				LPWSTR Complete = (LPWSTR)malloc(512 * sizeof(WCHAR));
				if (Complete == 0) break;
				wsprintfW(Complete, L"%s\\%s", lpSystemCertificatesPath, SubKeyName);
				if (RegDelnodeW(RootKey, Complete, TRUE)) {
					//std::wcout << "Rogue Key Deleted! \"" << Complete << "\"" << std::endl; // TODO: Why does this break the console?
					std::cout << "Rogue key is deleted!" << std::endl;
					Index--; // Because index has moved due to the deletion
				}
				else {
					std::cout << "Rogue key deletion failed!" << std::endl;
				}
				free(Complete);
			}
		}

		free(SubKeyName);
	}
	RegCloseKey(hRegSystemCertificates);
	return TRUE;
}

//*************************************************************
//
//  DeleteRustDeskTestCertsW()
//
//  Purpose:    Deletes RustDesk Test certificates and wrong key stores
//
//  Parameters: None
//
//  Return:     None
//
//*************************************************************

extern "C" void DeleteRustDeskTestCertsW() {
	// Current user
	std::wcout << "*** Current User" << std::endl;
	DeleteRustDeskTestCertsW_SingleHive(HKEY_CURRENT_USER);

	// Local machine (requires admin rights)
	std::wcout << "*** Local Machine" << std::endl;
	DeleteRustDeskTestCertsW_SingleHive(HKEY_LOCAL_MACHINE);

	// Iterate through all users (requires admin rights)
	LPCWSTR lpRoot = L"";
	HKEY hRegUsers;
	LONG res = RegOpenKeyExW(HKEY_USERS, lpRoot, NULL, KEY_READ, &hRegUsers);
	if (res != ERROR_SUCCESS) return;
	for (DWORD Index = 0; ; Index++) {
		LPWSTR SubKeyName = (LPWSTR)malloc(255 * sizeof(WCHAR));
		if (SubKeyName == 0) break;
		DWORD cName = 255;
		LONG res = RegEnumKeyExW(hRegUsers, Index, SubKeyName, &cName, NULL, NULL, NULL, NULL);
		if ((res != ERROR_SUCCESS) || (SubKeyName == NULL))
			break;
		std::wcout << "*** User: " << SubKeyName << std::endl;
		DeleteRustDeskTestCertsW_SingleHive(HKEY_USERS, SubKeyName);
	}
	RegCloseKey(hRegUsers);
}

//  int main()
//  {
//  	DeleteRustDeskTestCertsW();
//  	return 0;
//  }
