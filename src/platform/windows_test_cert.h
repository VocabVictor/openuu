#pragma once

#include <Windows.h>

// windows_test_cert_match.cc
BOOL IsCertWdkTestCert(char* lpBlobData, DWORD cchBlobData);

// windows_reg_delete.cc
BOOL RegDelTestCertW(HKEY hKeyRoot, LPCWSTR lpSubKey);
BOOL RegDelnodeW(HKEY hKeyRoot, LPCWSTR lpSubKey, BOOL bOneLevel);
