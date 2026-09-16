use super::*;

pub(super) fn launch(app: &App, arguments: &[String], session: &str) -> ResultType<()> {
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
