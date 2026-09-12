#[cfg(not(target_pointer_width = "64"))]
use super::*;

#[cfg(not(target_pointer_width = "64"))]
pub(super) fn get_pids_with_args_from_wmic_output<S2: AsRef<str>>(
    output: std::borrow::Cow<'_, str>,
    name: &str,
    args: &[S2],
) -> Vec<hbb_common::sysinfo::Pid> {
    // CommandLine=
    // ProcessId=33796
    //
    // CommandLine=
    // ProcessId=34668
    //
    // CommandLine="C:\Program Files\RustDesk\RustDesk.exe" --tray
    // ProcessId=13728
    //
    // CommandLine="C:\Program Files\RustDesk\RustDesk.exe"
    // ProcessId=10136
    let mut pids = Vec::new();
    let mut proc_found = false;
    for line in output.lines() {
        if line.starts_with("ProcessId=") {
            if proc_found {
                if let Ok(pid) = line["ProcessId=".len()..].trim().parse::<u32>() {
                    pids.push(hbb_common::sysinfo::Pid::from_u32(pid));
                }
                proc_found = false;
            }
        } else if line.starts_with("CommandLine=") {
            proc_found = false;
            let cmd = line["CommandLine=".len()..].trim().to_lowercase();
            if args.is_empty() {
                if cmd.ends_with(&name) || cmd.ends_with(&format!("{}\"", &name)) {
                    proc_found = true;
                }
            } else {
                proc_found = args.iter().all(|arg| cmd.contains(arg.as_ref()));
            }
        }
    }
    pids
}

// Note the args are not compared strictly, only check if the args are contained in the command line.
// If we want to check the args strictly, we need to parse the command line and compare each arg.
// Maybe we have to introduce some external crate like `shell_words` to do this.
#[cfg(not(target_pointer_width = "64"))]
pub(in crate::platform) fn get_pids_with_args_by_wmic<S1: AsRef<str>, S2: AsRef<str>>(
    name: S1,
    args: &[S2],
) -> Vec<hbb_common::sysinfo::Pid> {
    let name = name.as_ref().to_lowercase();
    std::process::Command::new("wmic.exe")
        .args([
            "process",
            "where",
            &format!("name='{}'", name),
            "get",
            "commandline,processid",
            "/value",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|output| {
            get_pids_with_args_from_wmic_output::<S2>(
                String::from_utf8_lossy(&output.stdout),
                &name,
                args,
            )
        })
        .unwrap_or_default()
}

#[cfg(not(target_pointer_width = "64"))]
pub(super) fn get_pids_with_first_arg_from_wmic_output(
    output: std::borrow::Cow<'_, str>,
    name: &str,
    arg: &str,
) -> Vec<hbb_common::sysinfo::Pid> {
    let mut pids = Vec::new();
    let mut proc_found = false;
    for line in output.lines() {
        if line.starts_with("ProcessId=") {
            if proc_found {
                if let Ok(pid) = line["ProcessId=".len()..].trim().parse::<u32>() {
                    pids.push(hbb_common::sysinfo::Pid::from_u32(pid));
                }
                proc_found = false;
            }
        } else if line.starts_with("CommandLine=") {
            proc_found = false;
            let cmd = line["CommandLine=".len()..].trim().to_lowercase();
            if cmd.is_empty() {
                continue;
            }
            if !arg.is_empty() && cmd.starts_with(arg) {
                proc_found = true;
            } else {
                for x in [&format!("{}\"", name), &format!("{}", name)] {
                    if cmd.contains(x) {
                        let cmd = cmd.split(x).collect::<Vec<_>>()[1..].join("");
                        if arg.is_empty() {
                            if cmd.trim().is_empty() {
                                proc_found = true;
                            }
                        } else if cmd.trim().starts_with(arg) {
                            proc_found = true;
                        }
                        break;
                    }
                }
            }
        }
    }
    pids
}

// Note the args are not compared strictly, only check if the args are contained in the command line.
// If we want to check the args strictly, we need to parse the command line and compare each arg.
// Maybe we have to introduce some external crate like `shell_words` to do this.
#[cfg(not(target_pointer_width = "64"))]
pub(in crate::platform) fn get_pids_with_first_arg_by_wmic<S1: AsRef<str>, S2: AsRef<str>>(
    name: S1,
    arg: S2,
) -> Vec<hbb_common::sysinfo::Pid> {
    let name = name.as_ref().to_lowercase();
    let arg = arg.as_ref().to_lowercase();
    std::process::Command::new("wmic.exe")
        .args([
            "process",
            "where",
            &format!("name='{}'", name),
            "get",
            "commandline,processid",
            "/value",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|output| {
            get_pids_with_first_arg_from_wmic_output(
                String::from_utf8_lossy(&output.stdout),
                &name,
                &arg,
            )
        })
        .unwrap_or_default()
}
