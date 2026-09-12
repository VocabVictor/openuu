use super::*;

// Builds a blob in the same layout generate.py writes, so these tests pin the
// cross-language format contract as well as the merge rules.
fn blob(files: &[(&str, &[u8])], exe: &str) -> &'static [u8] {
    let mut out = Vec::new();
    out.extend_from_slice(IDENTIFIER);
    for (path, data) in files {
        out.extend_from_slice(&(path.len() as u32).to_be_bytes());
        out.extend_from_slice(path.as_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(data);
        out.extend_from_slice(&[b'a'; MD5_LENGTH]);
    }
    out.extend_from_slice(IDENTIFIER);
    out.extend_from_slice(exe.as_bytes());
    Box::leak(out.into_boxed_slice())
}

fn entry<'a>(files: &'a [BinaryData], path: &str) -> Option<&'a BinaryData> {
    files
        .iter()
        .find(|file| normalize_path(&file.path) == normalize_path(path))
}

#[test]
fn parses_the_generate_py_layout() {
    let (files, exe) = parse(blob(
        &[("./rustdesk.exe", b"app"), ("./custom.txt", b"cfg")],
        "./rustdesk.exe",
    ))
    .unwrap();
    assert_eq!(exe, "./rustdesk.exe");
    assert_eq!(files.len(), 2);
    assert_eq!(entry(&files, "./custom.txt").unwrap().raw, b"cfg");
}

#[test]
fn rejects_malformed_blobs() {
    assert!(parse(b"".as_slice()).is_none());
    assert!(parse(b"notrustd".as_slice()).is_none());
    // Truncated mid-record rather than panicking on a slice out of range.
    assert!(parse(b"rustdesk\x00\x00\x00\x40partial".as_slice()).is_none());
}

#[test]
fn distinguishes_an_absent_package_from_a_malformed_one() {
    assert!(parse_package_blob(None).unwrap().0.is_empty());
    assert!(parse_package_blob(Some(b"damaged")).is_err());
    assert!(parse_package_blob(Some(blob(&[("./custom.txt", b"cfg")], ""))).is_err());
}

#[test]
fn without_a_package_the_stock_payload_is_untouched() {
    let embedded = parse(blob(&[("./rustdesk.exe", b"app")], "./rustdesk.exe")).unwrap();
    let (files, exe) = merge(embedded, Default::default());
    assert_eq!(exe, "./rustdesk.exe");
    assert!(entry(&files, "./rustdesk.exe").is_some());
}

#[test]
fn renames_the_stock_executable_to_the_package_name() {
    // x86: the big executable stays in the generic payload and only gets renamed.
    let embedded = parse(blob(
        &[("./rustdesk.exe", b"app"), ("./sciter.dll", b"dll")],
        "./rustdesk.exe",
    ))
    .unwrap();
    let package = parse(blob(&[("./custom.txt", b"cfg")], "./acme.exe")).unwrap();

    let (files, exe) = merge(embedded, package);

    assert_eq!(exe, "./acme.exe");
    assert!(entry(&files, "./acme.exe").is_some());
    assert!(entry(&files, "./rustdesk.exe").is_none());
    // Untouched neighbours survive.
    assert_eq!(entry(&files, "./sciter.dll").unwrap().raw, b"dll");
    assert_eq!(entry(&files, "./custom.txt").unwrap().raw, b"cfg");
}

#[test]
fn package_entries_win_over_the_generic_payload() {
    // x64: the customized executable and icons ship in the package instead.
    let embedded = parse(blob(
        &[
            ("./data/flutter_assets/assets/icon.ico", b"stock-icon"),
            ("./librustdesk.dll", b"core"),
        ],
        "./rustdesk.exe",
    ))
    .unwrap();
    let package = parse(blob(
        &[
            ("./acme.exe", b"branded"),
            ("./data/flutter_assets/assets/icon.ico", b"acme-icon"),
        ],
        "./acme.exe",
    ))
    .unwrap();

    let (files, exe) = merge(embedded, package);

    assert_eq!(exe, "./acme.exe");
    assert_eq!(
        entry(&files, "./data/flutter_assets/assets/icon.ico")
            .unwrap()
            .raw,
        b"acme-icon"
    );
    assert_eq!(
        files
            .iter()
            .filter(|f| normalize_path(&f.path) == "data/flutter_assets/assets/icon.ico")
            .count(),
        1
    );
    assert_eq!(entry(&files, "./librustdesk.dll").unwrap().raw, b"core");
}

#[test]
fn package_paths_are_recorded_for_the_dropped_file_sweep() {
    let package = parse(blob(
        &[("./custom.txt", b"cfg"), ("./data/logo.png", b"img")],
        "./acme.exe",
    ))
    .unwrap();
    let mut paths: Vec<String> = package.0.iter().map(|f| f.path.clone()).collect();
    paths.sort();
    assert_eq!(paths, vec!["./custom.txt", "./data/logo.png"]);

    // Merging must not disturb them: the generic payload contributes none.
    let embedded = parse(blob(&[("./librustdesk.dll", b"core")], "./rustdesk.exe")).unwrap();
    let (files, _) = merge(embedded, package);
    assert!(entry(&files, "./data/logo.png").is_some());
}

#[test]
fn matches_paths_across_separator_styles() {
    // generate.py emits backslashes when it runs on Windows.
    let embedded = parse(blob(&[(".\\rustdesk.exe", b"app")], ".\\rustdesk.exe")).unwrap();
    let package = parse(blob(&[("./custom.txt", b"cfg")], "./acme.exe")).unwrap();

    let (files, exe) = merge(embedded, package);

    assert_eq!(exe, "./acme.exe");
    assert!(entry(&files, "./acme.exe").is_some());
    assert!(entry(&files, ".\\rustdesk.exe").is_none());
}
