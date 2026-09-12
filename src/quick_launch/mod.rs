use hbb_common::{anyhow::{anyhow, bail}, ResultType};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::Command;

mod catalog;
use catalog::*;
mod launch;
use launch::*;

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
