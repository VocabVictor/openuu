use super::*;

    #[test]
    fn untrusted_peer_id_validation() {
        let cases = [
            ("123456789", true),
            ("m\u{00FC}nchen-pc", true),
            ("192.168.1.10:21118", true),
            ("9123456234@public", true),
            (
                r#"1" & oWS.Run("cmd.exe /k whoami /priv",1,False) & ""#,
                false,
            ),
            ("", false),
            ("peer id", false),
            ("peer\nid", false),
            ("peer/id", false),
            ("peer?id", false),
        ];

        for (id, expected) in cases {
            assert_eq!(is_valid_untrusted_peer_id(id), expected, "{id:?}");
        }
    }

    // ThrottledInterval tick at the same time as tokio interval, if no sleeps
    #[allow(non_snake_case)]

    #[test]
    fn test_mouse_event_constants_and_mask_layout() {
        use super::input::*;

        // Verify MOUSE_TYPE constants are unique and within the mask range.
        let types = [
            MOUSE_TYPE_MOVE,
            MOUSE_TYPE_DOWN,
            MOUSE_TYPE_UP,
            MOUSE_TYPE_WHEEL,
            MOUSE_TYPE_TRACKPAD,
            MOUSE_TYPE_MOVE_RELATIVE,
        ];

        let mut seen = std::collections::HashSet::new();
        for t in types.iter() {
            assert!(seen.insert(*t), "Duplicate mouse type: {}", t);
            assert_eq!(
                *t & MOUSE_TYPE_MASK,
                *t,
                "Mouse type {} exceeds mask {}",
                t,
                MOUSE_TYPE_MASK
            );
        }

        // The mask layout is: lower 3 bits for type, upper bits for buttons (shifted by 3).
        let combined_mask = MOUSE_TYPE_DOWN | ((MOUSE_BUTTON_LEFT | MOUSE_BUTTON_RIGHT) << 3);
        assert_eq!(combined_mask & MOUSE_TYPE_MASK, MOUSE_TYPE_DOWN);
        assert_eq!(combined_mask >> 3, MOUSE_BUTTON_LEFT | MOUSE_BUTTON_RIGHT);
    }
