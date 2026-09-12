use super::*;

pub(super) fn normalize_msi_product_code(value: &str) -> Option<String> {
    let value = value.trim().trim_matches('"');
    let value = value.strip_prefix('{')?.strip_suffix('}')?;
    let product_code = uuid::Uuid::parse_str(value).ok()?;
    Some(format!("{{{}}}", product_code.hyphenated()).to_uppercase())
}

pub(super) fn build_msi_uninstall_command(product_code: &str) -> String {
    format!(
        "set \"RUSTDESK_MSI_EXIT_CODE=\"\n\
MsiExec.exe /X {product_code} /norestart REBOOT=ReallySuppress\n\
set \"RUSTDESK_MSI_EXIT_CODE=%ERRORLEVEL%\"\n\
if \"%RUSTDESK_MSI_EXIT_CODE%\"==\"{MSI_EXIT_SUCCESS_REBOOT_REQUIRED}\" echo MSI uninstall succeeded with a reboot recommendation; continuing without reboot.\n\
if \"%RUSTDESK_MSI_EXIT_CODE%\"==\"{MSI_EXIT_SUCCESS_REBOOT_INITIATED}\" echo MSI uninstall succeeded with a reboot request; continuing without forcing reboot.\n\
if not \"%RUSTDESK_MSI_EXIT_CODE%\"==\"0\" if not \"%RUSTDESK_MSI_EXIT_CODE%\"==\"{MSI_EXIT_SUCCESS_REBOOT_REQUIRED}\" if not \"%RUSTDESK_MSI_EXIT_CODE%\"==\"{MSI_EXIT_SUCCESS_REBOOT_INITIATED}\" exit /b %RUSTDESK_MSI_EXIT_CODE%\n\
ver > nul"
    )
}

pub(super) fn get_reg_string_of(subkey: &str, name: &str) -> ResultType<Option<String>> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = subkey.strip_prefix(HKLM_PREFIX).unwrap_or(subkey);
    let key = match hklm.open_subkey(path) {
        Ok(key) => key,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => bail!("Failed to open registry key {subkey}: {err}"),
    };
    match key.get_value::<String, _>(name) {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => bail!("Failed to read {name} from registry key {subkey}: {err}"),
    }
}

pub(super) fn get_windows_installer_state(subkey: &str) -> ResultType<Option<bool>> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = subkey.strip_prefix(HKLM_PREFIX).unwrap_or(subkey);
    let key = match hklm.open_subkey(path) {
        Ok(key) => key,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => bail!("Failed to open registry key {subkey}: {err}"),
    };
    match key.get_value::<u32, _>(REG_NAME_WINDOWS_INSTALLER) {
        Ok(value) => Ok(Some(value == MSI_WINDOWS_INSTALLER_VALUE)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => bail!("Failed to read {REG_NAME_WINDOWS_INSTALLER} from {subkey}: {err}"),
    }
}

pub(super) fn parse_msi_product_code_from_uninstall_string(
    uninstall_string: &str,
    subkey: &str,
) -> ResultType<Option<String>> {
    if !uninstall_string
        .to_ascii_lowercase()
        .contains("msiexec.exe")
    {
        return Ok(None);
    }
    let start = uninstall_string
        .rfind('{')
        .ok_or_else(|| anyhow!("MSI uninstall string has no product code in {subkey}"))?;
    let end = uninstall_string
        .rfind('}')
        .ok_or_else(|| anyhow!("MSI uninstall string has no product code in {subkey}"))?;
    if start >= end {
        bail!("Invalid MSI uninstall string in {subkey}");
    }
    let product_code = uninstall_string
        .get(start..=end)
        .and_then(normalize_msi_product_code)
        .ok_or_else(|| anyhow!("Invalid MSI uninstall string in {subkey}"))?;
    Ok(Some(product_code))
}

pub(super) fn get_msi_product_code(subkey: &str, installer_state: Option<bool>) -> ResultType<Option<String>> {
    if installer_state == Some(false) {
        return Ok(None);
    }
    let product_code = get_reg_string_of(subkey, REG_NAME_MSI_PRODUCT_CODE)?;
    if let Some(product_code) = product_code.filter(|value| !value.is_empty()) {
        return normalize_msi_product_code(&product_code)
            .map(Some)
            .ok_or_else(|| anyhow!("Invalid MSI product code in {subkey}"));
    }

    let uninstall_string =
        get_reg_string_of(subkey, REG_NAME_UNINSTALL_STRING)?.unwrap_or_default();
    match parse_msi_product_code_from_uninstall_string(&uninstall_string, subkey)? {
        Some(product_code) => Ok(Some(product_code)),
        None if installer_state == Some(true) => {
            msi_registry::find_product_code(&crate::get_app_name())
        }
        None => Ok(None),
    }
}

pub(super) fn is_msi_uninstall_entry_in_view(subkey: &str, wow: bool, app_name: &str) -> ResultType<bool> {
    let flags = KEY_READ
        | if wow {
            KEY_WOW64_32KEY
        } else {
            KEY_WOW64_64KEY
        };
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = subkey.strip_prefix(HKLM_PREFIX).unwrap_or(subkey);
    let key = match hklm.open_subkey_with_flags(path, flags) {
        Ok(key) => key,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(anyhow!("Failed to open registry key {subkey}: {err}")),
    };
    msi_registry::is_matching_entry(&key, app_name, subkey)
}

pub(super) fn get_msi_uninstall_subkey(product_code: &str) -> ResultType<String> {
    let app_name = crate::get_app_name();
    let subkey = get_subkey(product_code, false);
    if is_msi_uninstall_entry_in_view(&subkey, false, &app_name)? {
        return Ok(subkey);
    }
    if is_msi_uninstall_entry_in_view(&subkey, true, &app_name)? {
        return Ok(get_subkey(product_code, true));
    }
    bail!("Matching native MSI uninstall entry {product_code} was not found")
}

pub(super) fn get_reg_msi_key(subkey: &str, is_msi: Option<bool>) -> ResultType<Option<String>> {
    // Only proceed if it's a custom client and MSI is installed.
    // `is_msi.unwrap_or(true)` is intentional: subsequent code validates the registry,
    // hence no early return is required upon MSI detection failure.
    if !(crate::common::is_custom_client() && is_msi.unwrap_or(true)) {
        return Ok(None);
    }

    let Some(product_code) = get_msi_product_code(subkey, is_msi)? else {
        if is_msi == Some(true) {
            bail!("MSI product code was not found in {subkey}");
        }
        return Ok(None);
    };
    Ok(Some(get_msi_uninstall_subkey(&product_code)?))
}

// Don't launch tray app when running with `\qn`.
// 1. Because `/qn` requires administrator permission and the tray app should be launched with user permission.
//   Or launching the main window from the tray app will cause the main window to be launched with administrator permission.
// 2. We are not able to launch the tray app if the UI is in the login screen.
// `fn update_me()` can handle the above cases, but for msi update, we need to do more work to handle the above cases.
//    1. Record the tray app session ids.
//    2. Do the update.
//    3. Restore the tray app sessions.
//    `1` and `3` must be done in custom actions.
//    We need also to handle the command line parsing to find the tray processes.
pub fn update_me_msi(msi: &str, quiet: bool) -> ResultType<()> {
    let quiet_args = if quiet { " /qn LAUNCH_TRAY_APP=N" } else { "" };
    let cmds =
        format!("chcp 65001 && msiexec /i \"{msi}\"{quiet_args} REBOOT=ReallySuppress /norestart");
    run_cmds(cmds, false, "update-msi")?;
    Ok(())
}

pub fn is_msi_installed() -> std::io::Result<bool> {
    let (subkey, _, _, _) = get_install_info();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let install_key = hklm.open_subkey(subkey.strip_prefix(HKLM_PREFIX).unwrap_or(&subkey))?;
    Ok(MSI_WINDOWS_INSTALLER_VALUE
        == install_key.get_value::<u32, _>(REG_NAME_WINDOWS_INSTALLER)?)
}
