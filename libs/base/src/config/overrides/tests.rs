use super::*;

const A_SETTING: &str = keys::OPTION_ENABLE_UDP_PUNCH;

#[test]
fn a_variable_name_is_the_key_shouted() {
    assert_eq!(env_name("allow-d3d-render"), "OPENUU_OPT_ALLOW_D3D_RENDER");
    assert_eq!(env_name("disable-udp"), "OPENUU_OPT_DISABLE_UDP");
}

#[test]
fn both_spellings_of_the_flag_are_taken_and_the_rest_is_left_alone() {
    let args = vec![
        "--server".to_owned(),
        "--option".to_owned(),
        "disable-udp=N".to_owned(),
        "--option=view_style=original".to_owned(),
        "--connect".to_owned(),
    ];
    let (found, rest) = take_from_command_line(args);
    assert_eq!(
        found,
        vec![
            Override {
                key: "disable-udp".to_owned(),
                value: "N".to_owned(),
                source: Source::CommandLine
            },
            Override {
                key: "view_style".to_owned(),
                value: "original".to_owned(),
                source: Source::CommandLine
            },
        ]
    );
    // Everything the caller still has to act on survives, in order.
    assert_eq!(rest, vec!["--server".to_owned(), "--connect".to_owned()]);
}

#[test]
fn a_value_with_an_equals_sign_in_it_keeps_it() {
    let (found, _) = take_from_command_line(vec!["--option".to_owned(), "k=a=b".to_owned()]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].value, "a=b");
}

#[test]
fn a_flag_without_a_value_is_dropped_rather_than_eating_the_next_argument() {
    let (found, rest) = take_from_command_line(vec!["--option".to_owned()]);
    assert!(found.is_empty());
    assert!(rest.is_empty());
}

#[test]
fn a_pair_that_is_not_key_equals_value_is_refused() {
    let (found, _) = take_from_command_line(vec!["--option".to_owned(), "nonsense".to_owned()]);
    assert!(found.is_empty());
    let (found, _) = take_from_command_line(vec!["--option".to_owned(), "=novalue".to_owned()]);
    assert!(found.is_empty());
}

#[test]
fn an_unknown_key_is_refused_because_it_has_no_table() {
    assert!(!apply_one(&Override {
        key: "not-a-real-key".to_owned(),
        value: "Y".to_owned(),
        source: Source::CommandLine,
    }));
}

#[test]
fn a_secret_is_refused() {
    assert!(!apply_one(&Override {
        key: provision::KEY_PERMANENT_PASSWORD.to_owned(),
        value: "hunter2".to_owned(),
        source: Source::Environment,
    }));
}

/// The one that matters: an override has to beat the value the machine has
/// stored, or it is useless for answering "what does this build do if".
#[test]
fn an_override_wins_over_the_stored_value() {
    config::LocalConfig::set_option(A_SETTING.to_owned(), "N".to_owned());
    assert_eq!(config::LocalConfig::get_option(A_SETTING), "N");
    assert!(apply_one(&Override {
        key: A_SETTING.to_owned(),
        value: "Y".to_owned(),
        source: Source::Environment,
    }));
    assert_eq!(config::LocalConfig::get_option(A_SETTING), "Y");
    config::OVERWRITE_LOCAL_SETTINGS.write().unwrap().remove(A_SETTING);
    config::LocalConfig::set_option(A_SETTING.to_owned(), String::new());
}

/// Command line and environment land in one table, so the order they are
/// written in is the whole of the rule between them.
#[test]
fn the_environment_wins_over_the_command_line() {
    let key = keys::OPTION_VIEW_STYLE;
    apply_one(&Override {
        key: key.to_owned(),
        value: "original".to_owned(),
        source: Source::CommandLine,
    });
    apply_one(&Override {
        key: key.to_owned(),
        value: "adaptive".to_owned(),
        source: Source::Environment,
    });
    assert_eq!(
        config::OVERWRITE_DISPLAY_SETTINGS
            .read()
            .unwrap()
            .get(key)
            .map(String::as_str),
        Some("adaptive")
    );
    config::OVERWRITE_DISPLAY_SETTINGS.write().unwrap().remove(key);
}
