use super::*;

#[test]
pub(super) fn session_scope_login_options_are_limited_to_authenticated_session_type() {
    let mut option = OptionMessage::new();
    option.image_quality = ImageQuality::Balanced.into();
    option.disable_audio = BoolOption::Yes.into();
    option.block_input = BoolOption::Yes.into();
    option.privacy_mode = BoolOption::Yes.into();

    let (scoped, violation) =
        Connection::scoped_login_option(AuthConnType::ViewCamera, &option);
    let scoped = scoped.unwrap();
    assert_eq!(violation, Some("login.option"));
    assert_eq!(
        scoped.image_quality.enum_value(),
        Ok(ImageQuality::Balanced)
    );
    assert_eq!(scoped.disable_audio.enum_value(), Ok(BoolOption::Yes));
    assert_eq!(scoped.block_input.enum_value(), Ok(BoolOption::NotSet));
    assert_eq!(scoped.privacy_mode.enum_value(), Ok(BoolOption::NotSet));

    let (scoped, violation) =
        Connection::scoped_login_option(AuthConnType::FileTransfer, &option);
    assert!(scoped.is_none());
    assert_eq!(violation, Some("login.option"));
}

#[test]
pub(super) fn session_scope_limited_render_noop_options_reject_mixed_fields() {
    for conn_type in [
        AuthConnType::FileTransfer,
        AuthConnType::Terminal,
        AuthConnType::PortForward,
    ] {
        let supported_decoding_only = option_msg(set_supported_decoding);
        assert_eq!(
            Connection::authorized_message_scope_violation(conn_type, &supported_decoding_only),
            None
        );

        let mixed_option = option_msg(|o| {
            set_supported_decoding(o);
            o.disable_audio = BoolOption::Yes.into();
        });
        assert_eq!(
            Connection::authorized_message_scope_violation(conn_type, &mixed_option),
            Some("misc.option")
        );
    }
}

#[test]
pub(super) fn session_scope_view_camera_options_keep_only_camera_fields() {
    let mut option = OptionMessage::new();
    option.image_quality = ImageQuality::Balanced.into();
    option.custom_image_quality = 80;
    option.custom_fps = 24;
    set_supported_decoding(&mut option);
    option.disable_audio = BoolOption::Yes.into();
    option.block_input = BoolOption::Yes.into();
    option.disable_clipboard = BoolOption::Yes.into();
    option.enable_file_transfer = BoolOption::Yes.into();
    option.terminal_persistent = BoolOption::Yes.into();

    let (scoped, violation) =
        Connection::scoped_login_option(AuthConnType::ViewCamera, &option);
    let scoped = scoped.unwrap();
    assert_eq!(violation, Some("login.option"));
    assert_eq!(
        scoped.image_quality.enum_value(),
        Ok(ImageQuality::Balanced)
    );
    assert_eq!(scoped.custom_image_quality, 80);
    assert_eq!(scoped.custom_fps, 24);
    assert!(scoped.supported_decoding.is_some());
    assert_eq!(scoped.disable_audio.enum_value(), Ok(BoolOption::Yes));
    assert_eq!(scoped.block_input.enum_value(), Ok(BoolOption::NotSet));
    assert_eq!(
        scoped.disable_clipboard.enum_value(),
        Ok(BoolOption::NotSet)
    );
    assert_eq!(
        scoped.enable_file_transfer.enum_value(),
        Ok(BoolOption::NotSet)
    );
    assert_eq!(
        scoped.terminal_persistent.enum_value(),
        Ok(BoolOption::NotSet)
    );
}
#[test]
pub(super) fn only_a_newer_remote_control_of_the_same_session_keeps_the_screen_unlocked() {
    let replaced_by = super::super::raii::AuthedConnID::is_newer_session_remote;

    let key = |session_id, peer: &str| SessionKey {
        peer_id: peer.to_owned(),
        name: "".to_owned(),
        session_id,
    };
    let conn = |conn_id, conn_type, session_key| AuthedConn {
        conn_id,
        conn_type,
        session_key,
        sender: mpsc::unbounded_channel().0,
    };
    let mine = key(7, "peer");
    let remote = AuthConnType::Remote;

    assert!(replaced_by(&conn(3, remote, mine.clone()), 2, &mine));
    // An older one, and itself: of connections ending at once only the last still locks.
    assert!(!replaced_by(&conn(1, remote, mine.clone()), 2, &mine));
    assert!(!replaced_by(&conn(2, remote, mine.clone()), 2, &mine));
    // A kind that keeps no screen in use.
    assert!(!replaced_by(
        &conn(3, AuthConnType::Terminal, mine.clone()),
        2,
        &mine
    ));
    // Another session of this peer, and another peer on the same session id: `SessionKey`
    // is all three fields, and either of those is someone else's screen to lock.
    assert!(!replaced_by(&conn(3, remote, key(8, "peer")), 2, &mine));
    assert!(!replaced_by(&conn(3, remote, key(7, "other")), 2, &mine));
}
