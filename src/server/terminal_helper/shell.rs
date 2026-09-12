use super::*;

/// Get the default shell for Windows.
pub fn get_default_shell() -> String {
    // Try PowerShell Core first (absolute paths only)
    let pwsh_paths = [
        "pwsh.exe",
        r"C:\Program Files\PowerShell\7\pwsh.exe",
        r"C:\Program Files\PowerShell\6\pwsh.exe",
    ];

    for path in &pwsh_paths {
        if std::path::Path::new(path).exists() {
            log::debug!("Found PowerShell Core: {}", path);
            return path.to_string();
        }
    }

    // Try Windows PowerShell
    let powershell_path = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
    if std::path::Path::new(powershell_path).exists() {
        return powershell_path.to_string();
    }

    // Fallback to cmd.exe
    std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
}

pub(super) fn utf8_shell_args(shell: &str) -> Vec<String> {
    let name = std::path::Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(shell)
        .to_ascii_lowercase();

    if name == "cmd.exe" || name == "cmd" {
        return vec!["/K".to_string(), "chcp 65001 >NUL".to_string()];
    }

    if name == "pwsh.exe" || name == "pwsh" || name == "powershell.exe" {
        return vec![
            "-NoLogo".to_string(),
            "-NoExit".to_string(),
            "-Command".to_string(),
            "chcp.com 65001 > $null; [Console]::InputEncoding = [System.Text.Encoding]::UTF8; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8".to_string(),
        ];
    }

    Vec::new()
}

pub fn configure_utf8_shell_command(shell: &str, cmd: &mut CommandBuilder) {
    for arg in utf8_shell_args(shell) {
        cmd.arg(arg);
    }
}
