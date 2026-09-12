use super::*;

pub fn copy_raw_cmd(src_raw: &str, _raw: &str, _path: &str) -> ResultType<String> {
    let main_raw = format!(
        "XCOPY \"{}\" \"{}\" /Y /E /H /C /I /K /R /Z",
        PathBuf::from(src_raw)
            .parent()
            .ok_or(anyhow!("Can't get parent directory of {src_raw}"))?
            .to_string_lossy()
            .to_string(),
        _path
    );
    return Ok(main_raw);
}

pub fn copy_exe_cmd(src_exe: &str, exe: &str, path: &str) -> ResultType<String> {
    let main_exe = copy_raw_cmd(src_exe, exe, path)?;
    Ok(format!(
        "
        {main_exe}
        copy /Y \"{ORIGIN_PROCESS_EXE}\" \"{path}\\{broker_exe}\"
        ",
        ORIGIN_PROCESS_EXE = win_topmost_window::ORIGIN_PROCESS_EXE,
        broker_exe = win_topmost_window::INJECTED_PROCESS_EXE,
    ))
}

#[inline]
pub fn rename_exe_cmd(src_exe: &str, path: &str) -> ResultType<String> {
    let src_exe_filename = PathBuf::from(src_exe)
        .file_name()
        .ok_or(anyhow!("Can't get file name of {src_exe}"))?
        .to_string_lossy()
        .to_string();
    let app_name = crate::get_app_name();
    if src_exe_filename == format!("{app_name}.exe") {
        Ok("".to_owned())
    } else {
        Ok(format!(
            "
        move /Y \"{path}\\{src_exe_filename}\" \"{path}\\{app_name}.exe\"
        ",
        ))
    }
}

#[inline]
pub fn remove_meta_toml_cmd(is_msi: bool, path: &str) -> String {
    if is_msi && crate::is_custom_client() {
        format!(
            "
        del /F /Q \"{path}\\meta.toml\"
        ",
        )
    } else {
        "".to_owned()
    }
}

pub(super) fn write_vbs(cmds: String, tip: &str) -> ResultType<PathBuf> {
    const UTF16LE_BOM: &[u8] = &[0xFF, 0xFE];
    let mut tmp = std::env::temp_dir();
    if vec!["&", "@", "^"]
        .drain(..)
        .any(|s| tmp.to_string_lossy().to_string().contains(s))
    {
        if let Ok(dir) = user_accessible_folder() {
            tmp = dir;
        }
    }
    tmp.push(format!("{}_{}.vbs", crate::get_app_name(), tip));
    let mut file = fs::File::create(&tmp)?;
    let cmds = cmds.replace("\r\n", "\n").replace('\n', "\r\n");
    let mut utf16: Vec<u16> = cmds.encode_utf16().collect();
    file.write_all(UTF16LE_BOM)?;
    file.write_all(to_le(&mut utf16))?;
    file.sync_all()?;
    Ok(tmp)
}

pub(super) fn to_le(v: &mut [u16]) -> &[u8] {
    for b in v.iter_mut() {
        *b = b.to_le()
    }
    unsafe { v.align_to().1 }
}
