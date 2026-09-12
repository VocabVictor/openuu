use super::*;

pub(super) fn desktop() -> ResultType<(String, String, PathBuf)> {
    #[cfg(target_os = "windows")]
    {
        let sid = crate::platform::get_current_process_session_id().ok_or_else(|| anyhow!("No desktop session"))?;
        if sid == 0 { bail!("Sign in to a desktop user session first"); }
        let user = crate::platform::get_active_username();
        if user.is_empty() { bail!("No signed-in desktop user"); }
        let home = crate::platform::get_active_user_home().ok_or_else(|| anyhow!("User profile is unavailable"))?;
        return Ok((sid.to_string(), user, home));
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let uid = crate::platform::get_active_userid();
        let user = crate::platform::get_active_username();
        if uid.is_empty() || uid == "0" || user.is_empty() { bail!("Sign in to a graphical desktop first"); }
        let home = crate::platform::get_active_user_home().ok_or_else(|| anyhow!("User profile is unavailable"))?;
        #[cfg(target_os = "linux")]
        if std::env::var("DISPLAY").unwrap_or_default().is_empty() && std::env::var("WAYLAND_DISPLAY").unwrap_or_default().is_empty() {
            bail!("The desktop session environment is unavailable");
        }
        return Ok((uid, user, home));
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    bail!("Quick launch is only supported on desktop systems")
}

pub(super) fn scan(root: &Path, extension: &str, depth: usize, paths: &mut Vec<PathBuf>) {
    if depth == 0 || paths.len() >= 512 { return; }
    let Ok(entries) = std::fs::read_dir(root) else { return; };
    for entry in entries.flatten().take(4096) {
        if paths.len() >= 512 { break; }
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case(extension)).unwrap_or(false) {
            paths.push(path);
        } else if entry.file_type().map(|t| t.is_dir() && !t.is_symlink()).unwrap_or(false) {
            scan(&path, extension, depth - 1, paths);
        }
    }
}

pub(super) fn catalog(home: &Path) -> Vec<App> {
    let mut paths = Vec::new();
    #[cfg(target_os = "windows")]
    {
        if let Some(root) = std::env::var_os("PROGRAMDATA") {
            scan(&PathBuf::from(root).join("Microsoft/Windows/Start Menu/Programs"), "lnk", 8, &mut paths);
        }
        scan(&home.join("AppData/Roaming/Microsoft/Windows/Start Menu/Programs"), "lnk", 8, &mut paths);
    }
    #[cfg(target_os = "macos")]
    for root in [PathBuf::from("/Applications"), PathBuf::from("/System/Applications"), home.join("Applications")] {
        scan(&root, "app", 4, &mut paths);
    }
    #[cfg(target_os = "linux")]
    for root in [home.join(".local/share/applications"), PathBuf::from("/usr/local/share/applications"), PathBuf::from("/usr/share/applications"), PathBuf::from("/var/lib/flatpak/exports/share/applications"), home.join(".local/share/flatpak/exports/share/applications"), PathBuf::from("/var/lib/snapd/desktop/applications")] {
        scan(&root, "desktop", 5, &mut paths);
    }
    let mut apps = Vec::new();
    for path in paths {
        if path.as_os_str().len() > 2048 { continue; }
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        #[cfg(target_os = "linux")]
        let name = match desktop_name(&path) { Some(n) => n, None => continue };
        apps.push(App { id: path.to_string_lossy().into(), name,
            kind: path.extension().map(|s| s.to_string_lossy().to_ascii_lowercase()).unwrap_or_default() });
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a,b| a.id == b.id);
    apps
}

#[cfg(target_os = "linux")]
pub(super) fn desktop_name(path: &Path) -> Option<String> {
    if std::fs::metadata(path).ok()?.len() > 256 * 1024 { return None; }
    let text = std::fs::read_to_string(path).ok()?;
    let mut section = false;
    let mut name = None;
    let mut application = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') { section = line == "[Desktop Entry]"; continue; }
        if !section { continue; }
        if line == "Hidden=true" || line == "NoDisplay=true" { return None; }
        if line == "Type=Application" { application = true; }
        if let Some(v) = line.strip_prefix("Name=") { name = Some(v.to_string()); }
    }
    if application { name } else { None }
}

#[cfg(target_os = "windows")]
pub(super) fn quote_windows(arg: &str) -> String {
    let mut result = String::from("\"");
    let mut slashes = 0;
    for c in arg.chars() {
        if c == '\\' { slashes += 1; continue; }
        result.push_str(&"\\".repeat(if c == '"' { slashes * 2 + 1 } else { slashes }));
        result.push(c);
        slashes = 0;
    }
    result.push_str(&"\\".repeat(slashes * 2));
    result.push('"');
    result
}
