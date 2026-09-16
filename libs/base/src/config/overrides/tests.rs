use super::*;

/// Keys no other test in this crate asserts on. One crate shares these tables,
/// so borrowing a key another test checks fails that one, from another thread.
const LOCAL: &str = keys::OPTION_SYNC_AB_TAGS;
const DISPLAY: &str = keys::OPTION_SCROLL_STYLE;

fn env(key: &str, value: &str) -> Set {
    Set {
        key: key.to_owned(),
        value: value.to_owned(),
        from: From::Env,
    }
}

#[test]
fn var_is_the_key_shouted() {
    assert_eq!(var("allow-d3d-render"), "OPENUU_OPT_ALLOW_D3D_RENDER");
}

#[test]
fn both_flag_spellings_taken_rest_untouched() {
    let (found, rest) = from_args(vec![
        "--server".to_owned(),
        "--option".to_owned(),
        "disable-udp=N".to_owned(),
        "--option=view_style=original".to_owned(),
        "--connect".to_owned(),
    ]);
    assert_eq!(
        found.iter().map(|s| (&s.key[..], &s.value[..])).collect::<Vec<_>>(),
        [("disable-udp", "N"), ("view_style", "original")]
    );
    assert_eq!(rest, ["--server", "--connect"]);
}

#[test]
fn value_keeps_its_own_equals_sign() {
    let (found, _) = from_args(vec!["--option".to_owned(), "k=a=b".to_owned()]);
    assert_eq!(found[0].value, "a=b");
}

#[test]
fn flag_without_value_eats_nothing() {
    let (found, rest) = from_args(vec!["--option".to_owned()]);
    assert!(found.is_empty() && rest.is_empty());
}

#[test]
fn bad_pair_refused() {
    for bad in ["nonsense", "=novalue"] {
        let (found, _) = from_args(vec!["--option".to_owned(), bad.to_owned()]);
        assert!(found.is_empty(), "{}", bad);
    }
}

#[test]
fn unknown_key_has_no_table() {
    assert_eq!(set(&env("not-a-real-key", "Y")), Some("no such key"));
}

#[test]
fn secret_refused() {
    assert_eq!(
        set(&env(provision::KEY_PERMANENT_PASSWORD, "hunter2")),
        Some("secret")
    );
}

/// The one that matters: an override must beat what is under it, or it cannot
/// answer "what does this build do if".
///
/// The layer under it is faked in the default table rather than through
/// `LocalConfig::set_option`, which would write the config of whoever runs the
/// tests. What is under test is that the override lands in the table read first.
#[test]
fn override_beats_what_is_under_it() {
    config::DEFAULT_LOCAL_SETTINGS
        .write()
        .unwrap()
        .insert(LOCAL.to_owned(), "N".to_owned());
    assert_eq!(config::LocalConfig::get_option(LOCAL), "N");
    assert_eq!(set(&env(LOCAL, "Y")), None);
    assert_eq!(config::LocalConfig::get_option(LOCAL), "Y");
    config::OVERWRITE_LOCAL_SETTINGS.write().unwrap().remove(LOCAL);
    config::DEFAULT_LOCAL_SETTINGS.write().unwrap().remove(LOCAL);
}

/// Both land in one table, so the write order is the whole rule between them.
#[test]
fn env_beats_cli() {
    set(&Set {
        key: DISPLAY.to_owned(),
        value: "scrollbar".to_owned(),
        from: From::Cli,
    });
    set(&env(DISPLAY, "scrolledge"));
    assert_eq!(
        config::OVERWRITE_DISPLAY_SETTINGS.read().unwrap().get(DISPLAY).map(String::as_str),
        Some("scrolledge")
    );
    config::OVERWRITE_DISPLAY_SETTINGS.write().unwrap().remove(DISPLAY);
}
