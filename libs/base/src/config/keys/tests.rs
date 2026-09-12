/// The glob above and the constants below share one namespace, and Rust
/// silently prefers the explicit item over a glob import. A key defined on
/// both sides would therefore compile, with the client and the server
/// disagreeing about its string value and nothing to signal it. Keep the
/// two sets apart.
#[test]
fn key_names_do_not_collide_with_hbb_common() {
    fn names(src: &str) -> Vec<&str> {
        src.lines()
            .filter_map(|l| l.trim().strip_prefix("pub const "))
            .filter_map(|l| l.split(':').next())
            .map(str::trim)
            .filter(|n| n.starts_with("OPTION_") || n.starts_with("KEYS_"))
            .collect()
    }

    // every topic file of this module, so a key defined anywhere in it is seen
    let here = names(concat!(
        include_str!("options.rs"),
        include_str!("builtin.rs"),
        include_str!("connection.rs"),
        include_str!("local.rs"),
        include_str!("display_settings.rs"),
        include_str!("local_settings.rs"),
        include_str!("settings.rs"),
        include_str!("buildin_settings.rs"),
    ));
    let there = names(include_str!("../../../../hbb_common/src/config.rs"));
    assert!(
        !here.is_empty() && !there.is_empty(),
        "key parsing found nothing"
    );

    let both: Vec<_> = here.iter().filter(|n| there.contains(n)).collect();
    assert!(
        both.is_empty(),
        "defined in both crates, so the local one shadows hbb_common's \
         with no diagnostic: {:?}",
        both
    );
}
