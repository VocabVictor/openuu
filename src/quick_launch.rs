use hbb_common::{anyhow::{anyhow, bail}, ResultType};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::Command;

#[derive(Clone, Serialize, Deserialize)]
pub struct App {
    pub id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub request_id: String,
    pub operation: String,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub custom: bool,
}

pub fn handle(raw: &str) -> String {
    let mut request_id = String::new();
    let result = (|| -> ResultType<serde_json::Value> {
        if raw.len() > 32768 { bail!("Request is too large"); }
        let request: Request = serde_json::from_str(raw)?;
        request_id = request.request_id.clone();
        if request_id.len() > 128 || request.arguments.len() > 64 ||
            request.arguments.iter().any(|s| s.contains('\0') || s.len() > 4096) {
            bail!("Invalid request");
        }
        let (session, user, home) = desktop()?;
        if !request.user.is_empty() && request.user != user {
            bail!("The shortcut belongs to another desktop user");
        }
        if !request.session.is_empty() && request.session != session {
            bail!("Switch the remote desktop to the requested user session first");
        }
        match request.operation.as_str() {
            "list" => Ok(serde_json::json!({"apps": catalog(&home), "session": session, "user": user})),
            "launch" | "icon" => {
                let path = PathBuf::from(&request.app_id);
                if !path.is_absolute() || !path.exists() { bail!("Application was removed or the path is invalid"); }
                let app = if request.custom {
                    App { id: request.app_id.clone(), name: request.app_id.clone(), kind: "executable".into() }
                } else {
                    catalog(&home).into_iter().find(|a| a.id == request.app_id)
                        .ok_or_else(|| anyhow!("Application is no longer in the desktop catalog"))?
                };
                if request.operation == "icon" {
                    let icon = app_icon(&app).map(|bytes| hbb_common::base64::encode(bytes)).unwrap_or_default();
                    return Ok(serde_json::json!({"icon": icon}));
                }
                launch(&app, &request.arguments, &session)?;
                Ok(serde_json::json!({"accepted": true, "session": session}))
            }
            _ => bail!("Unsupported operation"),
        }
    })();
    match result {
        Ok(data) => serde_json::json!({"request_id": request_id, "data": data}).to_string(),
        Err(e) => serde_json::json!({"request_id": request_id, "error": e.to_string()}).to_string(),
    }
}

fn desktop() -> ResultType<(String, String, PathBuf)> {
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

fn scan(root: &Path, extension: &str, depth: usize, paths: &mut Vec<PathBuf>) {
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

fn catalog(home: &Path) -> Vec<App> {
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
fn desktop_name(path: &Path) -> Option<String> {
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
fn quote_windows(arg: &str) -> String {
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

fn launch(app: &App, arguments: &[String], session: &str) -> ResultType<()> {
    #[cfg(target_os = "windows")]
    {
        let sid: u32 = session.parse()?;
        let (exe, args) = if app.kind == "lnk" {
            if !arguments.is_empty() { bail!("Shortcut arguments are configured in the shortcut itself"); }
            let windows = std::env::var("WINDIR")?;
            (format!("{windows}\\explorer.exe"), vec![app.id.clone()])
        } else {
            if !app.id.to_lowercase().ends_with(".exe") { bail!("Choose an EXE or a catalog application"); }
            (app.id.clone(), arguments.to_vec())
        };
        if !crate::platform::is_root() {
            crate::platform::run_exe_direct(&exe, args.iter().map(String::as_str).collect(), true)?;
            return Ok(());
        }
        let quoted: Vec<_> = args.iter().map(|s| quote_windows(s)).collect();
        crate::platform::run_exe_in_session(&exe, quoted.iter().map(String::as_str).collect(), sid, true)?;
        return Ok(());
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let uid: u32 = session.parse()?;
        let current_uid = unsafe { hbb_common::libc::geteuid() };
        if current_uid != 0 && current_uid != uid { bail!("The selected desktop belongs to another user"); }
        let mut argv = Vec::<String>::new();
        #[cfg(target_os = "macos")]
        {
            if app.kind == "app" {
                argv.extend(["/usr/bin/open".into(), "-a".into(), app.id.clone()]);
                if !arguments.is_empty() { argv.push("--args".into()); argv.extend_from_slice(arguments); }
            } else { argv.push(app.id.clone()); argv.extend_from_slice(arguments); }
        }
        #[cfg(target_os = "linux")]
        {
            if app.kind == "desktop" {
                if !arguments.is_empty() { bail!("Desktop application arguments belong in its desktop entry"); }
                argv.extend(["/usr/bin/gio".into(), "launch".into(), app.id.clone()]);
            } else { argv.push(app.id.clone()); argv.extend_from_slice(arguments); }
        }
        let mut command;
        if current_uid == 0 {
            #[cfg(target_os = "macos")]
            { command = Command::new("/bin/launchctl"); command.args(["asuser", session, "/usr/bin/sudo", "-n", "-u", &format!("#{uid}"), "--"]); }
            #[cfg(target_os = "linux")]
            {
                command = Command::new("/usr/bin/sudo");
                command.args(["-n", "-u", &format!("#{uid}"), "--", "/usr/bin/env"]);
                command.arg(format!("XDG_RUNTIME_DIR=/run/user/{uid}"));
                command.arg(format!("DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{uid}/bus"));
                for key in ["DISPLAY", "WAYLAND_DISPLAY", "XAUTHORITY"] {
                    if let Ok(value) = std::env::var(key) { command.arg(format!("{key}={value}")); }
                }
            }
            command.args(&argv);
        } else { command = Command::new(&argv[0]); command.args(&argv[1..]); }
        if app.kind != "executable" {
            let status = command.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()?;
            if !status.success() { bail!("The desktop launcher rejected the application: {status}"); }
            return Ok(());
        }
        let mut child = command.spawn()?;
        // Reap without blocking the connection while an application stays open.
        std::thread::spawn(move || { let _ = child.wait(); });
        return Ok(());
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    bail!("Unsupported platform")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_oversized_request() { assert!(handle(&"x".repeat(32769)).contains("too large")); }
    #[test]
    fn rejects_unknown_fields() { assert!(handle(r#"{"request_id":"1","operation":"list","shell":"bad"}"#).contains("unknown field")); }
    #[test]
    fn denial_preserves_request_identity() {
        let reply: serde_json::Value = serde_json::from_str(&denied(r#"{"request_id":"abc","operation":"launch"}"#)).expect("JSON");
        assert_eq!(reply["request_id"], "abc");
        assert!(reply.get("data").is_none());
        assert!(reply.get("error").is_some());
    }
    #[test]
    fn rejects_null_in_arguments_before_launch() {
        let reply = handle(r#"{"request_id":"x","operation":"launch","arguments":["\u0000"]}"#);
        assert!(reply.contains("Invalid request"));
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn quotes_windows_arguments() {
        assert_eq!(quote_windows("a b"), "\"a b\"");
        assert_eq!(quote_windows("a\"b"), "\"a\\\"b\"");
        assert_eq!(quote_windows("C:\\x\\"), "\"C:\\x\\\\\"");
    }
}

#[cfg(target_os = "windows")]
fn app_icon(app: &App) -> Option<Vec<u8>> {
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::{shellapi::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON}, winuser::DestroyIcon};
    let path: Vec<u16> = std::ffi::OsStr::new(&app.id).encode_wide().chain(Some(0)).collect();
    let mut info: SHFILEINFOW = unsafe { std::mem::zeroed() };
    if unsafe { SHGetFileInfoW(path.as_ptr(), 0, &mut info, std::mem::size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_SMALLICON) } == 0 { return None; }
    let data = crate::platform::get_cursor_data(info.hIcon as u64);
    unsafe { DestroyIcon(info.hIcon); }
    let data = data.ok()?;
    let mut png = Vec::new();
    repng::encode(&mut png, data.width as u32, data.height as u32, &data.colors).ok()?;
    Some(png)
}

#[cfg(target_os = "linux")]
fn app_icon(app: &App) -> Option<Vec<u8>> {
    let text = std::fs::read_to_string(&app.id).ok()?;
    let icon = text.lines().find_map(|l| l.strip_prefix("Icon="))?;
    let home = crate::platform::get_active_user_home()?;
    let roots = [home.join(".local/share/icons"), PathBuf::from("/usr/share/icons"), PathBuf::from("/usr/share/pixmaps")];
    let mut paths = Vec::new();
    if Path::new(icon).is_absolute() { paths.push(PathBuf::from(icon)); }
    else if !icon.contains('/') {
        for root in &roots {
            paths.push(root.join(format!("{icon}.png")));
            for size in ["48x48", "64x64", "32x32", "128x128"] {
                paths.push(root.join(format!("hicolor/{size}/apps/{icon}.png")));
            }
        }
    }
    for path in paths {
        if path.as_os_str().len() > 2048 { continue; }
        let Ok(path) = path.canonicalize() else { continue; };
        if !roots.iter().any(|r| path.starts_with(r)) { continue; }
        if path.extension().and_then(|s| s.to_str()) != Some("png") { continue; }
        if std::fs::metadata(&path).ok()?.len() <= 256 * 1024 {
            return std::fs::read(path).ok();
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn app_icon(app: &App) -> Option<Vec<u8>> {
    let resources = Path::new(&app.id).join("Contents/Resources");
    let icon = std::fs::read_dir(resources).ok()?.flatten().map(|e| e.path())
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("icns"))?;
    let temp = std::env::temp_dir().join(format!("openuu-icon-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&temp).ok()?;
    let output = temp.join("icon.png");
    let result = (|| {
        let status = Command::new("/usr/bin/sips").args(["-s", "format", "png", "-z", "48", "48"])
            .arg(icon).arg("--out").arg(&output).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().ok()?;
        if !status.success() || std::fs::metadata(&output).ok()?.len() > 256 * 1024 { return None; }
        std::fs::read(&output).ok()
    })();
    let _ = std::fs::remove_file(&output);
    let _ = std::fs::remove_dir(&temp);
    result
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn app_icon(_: &App) -> Option<Vec<u8>> { None }

pub fn denied(raw: &str) -> String {
    let id = if raw.len() <= 32768 {
        serde_json::from_str::<Request>(raw).ok().map(|r| r.request_id).filter(|id| id.len() <= 128).unwrap_or_default()
    } else { String::new() };
    serde_json::json!({"request_id": id, "error": "Quick launch requires an authorized control session"}).to_string()
}
