use super::*;

#[test]
fn view_only_login_disables_remote_mutations() {
    let mut lc = LoginConfigHandler::default();
    lc.view_only_session = true;
    lc.config.view_only.v = false;
    lc.config.enable_file_copy_paste.v = true;
    lc.config.lock_after_session_end.v = true;
    let options = lc.get_option_message(false).expect("desktop options");
    assert_eq!(options.disable_keyboard.enum_value(), Ok(BoolOption::Yes));
    assert_eq!(options.disable_clipboard.enum_value(), Ok(BoolOption::Yes));
    assert_eq!(options.enable_file_transfer.enum_value(), Ok(BoolOption::No));
    assert_eq!(options.lock_after_session_end.enum_value(), Ok(BoolOption::No));
    assert!(lc.toggle_option("view-only".into()).is_none());
    assert!(lc.get_toggle_option("view-only"));
    assert!(!lc.config.view_only.v);
    assert!(lc.toggle_option("disable-clipboard".into()).is_none());
}
