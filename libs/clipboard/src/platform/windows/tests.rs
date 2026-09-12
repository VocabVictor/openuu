#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(deref_nullptr)]

use super::*;
use std::iter::once;

const FILE_NAME_CODE_UNITS: usize = 260;
const FILE_NAME_CASES: &[(&str, bool)] = &[
    ("", false),
    ("/absolute", false),
    ("C:\\absolute", false),
    ("dir\\..\\payload", false),
    ("dir//payload", false),
    ("file.", false),
    ("file ", false),
    (" report.txt", false),
    (" NUL.txt", false),
    ("dir\\ nested.txt", false),
    ("CON", false),
    ("nul.txt", false),
    ("dir\\AUX.log", false),
    ("PRN.tar.gz", false),
    ("com1", false),
    ("COM\u{00b9}.txt", false),
    ("COM\u{00b2}.txt", false),
    ("lpt9.log", false),
    ("dir/LPT\u{00b3}", false),
    ("CONIN$", false),
    ("dir\\conout$", false),
    ("CLOCK$", false),
    ("bad<name", false),
    ("bad>name", false),
    ("bad:name", false),
    ("bad\"name", false),
    ("bad|name", false),
    ("bad?name", false),
    ("bad*name", false),
    ("bad\u{0001}name", false),
    ("dir\\bad\u{001f}name", false),
    ("normal.txt", true),
    (".gitignore", true),
    ("dir\\nested file.txt", true),
    ("dir/nested file.txt", true),
    ("com10.txt", true),
    ("auxiliary.log", true),
    ("clock$.txt", true),
    ("conin$.txt", true),
];

fn file_descriptor_name_valid(name: &str) -> bool {
    let wide_name: Vec<_> = name.encode_utf16().chain(once(0)).collect();
    unsafe { wf_cliprdr_file_descriptor_name_valid(wide_name.as_ptr()) == TRUE }
}

#[test]
fn validates_file_descriptor_names() {
    for &(name, expected) in FILE_NAME_CASES {
        assert_eq!(
            file_descriptor_name_valid(name),
            expected,
            "unexpected validity for {name:?}"
        );
    }
}

#[test]
fn rejects_non_terminated_file_descriptor_name() {
    let wide_name = [WCHAR::from(b'a'); FILE_NAME_CODE_UNITS];
    assert_eq!(
        unsafe { wf_cliprdr_file_descriptor_name_valid(wide_name.as_ptr()) },
        FALSE
    );
}
