use super::*;

#[cfg(target_os = "windows")]
pub(super) fn app_icon(app: &App) -> Option<Vec<u8>> {
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
pub(super) fn app_icon(app: &App) -> Option<Vec<u8>> {
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
pub(super) fn app_icon(app: &App) -> Option<Vec<u8>> {
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
pub(super) fn app_icon(_: &App) -> Option<Vec<u8>> { None }
