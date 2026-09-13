// https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ics/c-adding-an-application-rule-edge-traversal

/********************************************************************
Copyright (C) Microsoft. All Rights Reserved.

Abstract:
    This C++ file includes sample code that adds a firewall rule with
 EdgeTraversalOptions (one of the EdgeTraversalOptions values).

********************************************************************/

#include "pch.h"
#include <windows.h>
#include <stdio.h>
#include <netfw.h>
#include <strsafe.h>

#pragma comment(lib, "ole32.lib")
#pragma comment(lib, "oleaut32.lib")

#define STRING_BUFFER_SIZE  500     

// Forward declarations
HRESULT    WFCOMInitialize(INetFwPolicy2** ppNetFwPolicy2);
void       WFCOMCleanup(INetFwPolicy2* pNetFwPolicy2);
HRESULT    RemoveFirewallRule(
    __in INetFwPolicy2* pNetFwPolicy2,
    __in LPWSTR exeName);
HRESULT    AddFirewallRuleWithEdgeTraversal(__in INetFwPolicy2* pNetFwPolicy2,
                                            __in bool in,
                                            __in LPWSTR exeName,
                                            __in LPWSTR exeFile);

bool AddFirewallRule(bool add, LPWSTR exeName, LPWSTR exeFile)
{
    bool result = false;
    HRESULT hrComInit = S_OK;
    HRESULT hr = S_OK;
    INetFwPolicy2* pNetFwPolicy2 = NULL;

    // Initialize COM.
    hrComInit = CoInitializeEx(
        0,
        COINIT_APARTMENTTHREADED
    );

    // Ignore RPC_E_CHANGED_MODE; this just means that COM has already been
    // initialized with a different mode. Since we don't care what the mode is,
    // we'll just use the existing mode.
    if (hrComInit != RPC_E_CHANGED_MODE)
    {
        if (FAILED(hrComInit))
        {
            WcaLog(LOGMSG_STANDARD, "CoInitializeEx failed: 0x%08lx\n", hrComInit);
            goto Cleanup;
        }
    }

    // Retrieve INetFwPolicy2
    hr = WFCOMInitialize(&pNetFwPolicy2);
    if (FAILED(hr))
    {
        goto Cleanup;
    }

    if (add) {
        // Add firewall rule with EdgeTraversalOption=DeferApp (Windows7+) if available 
        //   else add with Edge=True (Vista and Server 2008).
        hr = AddFirewallRuleWithEdgeTraversal(pNetFwPolicy2, true, exeName, exeFile);
        hr = AddFirewallRuleWithEdgeTraversal(pNetFwPolicy2, false, exeName, exeFile);
    }
    else {
        hr = RemoveFirewallRule(pNetFwPolicy2, exeName);
    }
    result = SUCCEEDED(hr);

Cleanup:

    // Release INetFwPolicy2
    WFCOMCleanup(pNetFwPolicy2);

    // Uninitialize COM.
    if (SUCCEEDED(hrComInit))
    {
        CoUninitialize();
    }

    return result;
}

BSTR MakeRuleName(__in LPWSTR exeName)
{
    WCHAR pwszTemp[STRING_BUFFER_SIZE] = L"";
    HRESULT hr = StringCchPrintfW(pwszTemp, STRING_BUFFER_SIZE, L"%ls Service", exeName);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed to compose a resource identifier string: 0x%08lx\n", hr);
        return NULL;
    }
    return SysAllocString(pwszTemp);
}

HRESULT    RemoveFirewallRule(
    __in INetFwPolicy2* pNetFwPolicy2,
    __in LPWSTR exeName)
{
    HRESULT hr = S_OK;
    INetFwRules* pNetFwRules = NULL;

    WCHAR pwszTemp[STRING_BUFFER_SIZE] = L"";

    BSTR RuleName = NULL;

    RuleName = MakeRuleName(exeName);
    if (NULL == RuleName)
    {
        WcaLog(LOGMSG_STANDARD, "\nERROR: Insufficient memory\n");
        goto Cleanup;
    }

    hr = pNetFwPolicy2->get_Rules(&pNetFwRules);
    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "Failed to retrieve firewall rules collection : 0x%08lx\n", hr);
        goto Cleanup;
    }

    // We need to "Remove()" twice, because both "in" and "out" rules are added?
    // There's no remarks for this case https://learn.microsoft.com/en-us/windows/win32/api/netfw/nf-netfw-inetfwrules-remove
    hr = pNetFwRules->Remove(RuleName);
    hr = pNetFwRules->Remove(RuleName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to remove firewall rule \"%ls\" : 0x%08lx\n", exeName, hr);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Firewall rule \"%ls\" is removed\n", exeName);
    }

Cleanup:

    SysFreeString(RuleName);

    if (pNetFwRules != NULL)
    {
        pNetFwRules->Release();
    }

    return hr;
}

// Instantiate INetFwPolicy2
HRESULT WFCOMInitialize(INetFwPolicy2** ppNetFwPolicy2)
{
    HRESULT hr = S_OK;

    hr = CoCreateInstance(
        __uuidof(NetFwPolicy2),
        NULL,
        CLSCTX_INPROC_SERVER,
        __uuidof(INetFwPolicy2),
        (void**)ppNetFwPolicy2);

    if (FAILED(hr))
    {
        WcaLog(LOGMSG_STANDARD, "CoCreateInstance for INetFwPolicy2 failed: 0x%08lx\n", hr);
        goto Cleanup;
    }

Cleanup:
    return hr;
}

// Release INetFwPolicy2
void WFCOMCleanup(INetFwPolicy2* pNetFwPolicy2)
{
    // Release the INetFwPolicy2 object (Vista+)
    if (pNetFwPolicy2 != NULL)
    {
        pNetFwPolicy2->Release();
    }
}
