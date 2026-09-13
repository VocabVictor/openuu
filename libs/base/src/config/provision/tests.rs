use super::apply::install;
use super::*;
use hbb_common::{
    base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine},
    config,
};

const FILE: &str = r#"{
  "version": 1,
  "server": {"id": "rs.test", "relay": "rs.test", "api": "https://rs.test:21114", "key": "pk=="},
  "options": {"verification-method": "use-permanent-password", "permanent-password": "s3cret",
              "no-such-key": "x"},
  "local": {"lang": "zh-CN"},
  "locked": ["custom-rendezvous-server", "verification-method"]
}"#;

fn clear(keys: &[&str]) {
    for table in [
        &*config::DEFAULT_SETTINGS,
        &*config::OVERWRITE_SETTINGS,
        &*config::DEFAULT_LOCAL_SETTINGS,
        &*config::OVERWRITE_LOCAL_SETTINGS,
    ] {
        let mut t = table.write().unwrap();
        for k in keys {
            t.remove(*k);
        }
    }
}

#[test]
fn parses_every_accepted_form() {
    let p = parse(FILE).unwrap();
    assert_eq!(p.version, 1);
    assert_eq!(p.server.id, "rs.test");
    assert_eq!(p.options["verification-method"], "use-permanent-password");
    assert_eq!(p.locked.len(), 2);

    let share = encode_share(&p).unwrap();
    assert!(share.starts_with(URI_PREFIX));
    let q = parse(&share).unwrap();
    assert_eq!(q.server, p.server);
    assert!(!q.options.contains_key("permanent-password"), "secrets never enter the payload");
    assert!(q.local.is_empty() && q.locked.is_empty());
    assert_eq!(parse(share.trim_start_matches(URI_PREFIX)).unwrap(), q);

    let legacy_json = r#"{"host":"rs.legacy","relay":"","api":"http://rs.legacy:21114","key":"k"}"#;
    let legacy: String = URL_SAFE_NO_PAD.encode(legacy_json).chars().rev().collect();
    for text in [legacy.clone(), format!("{LEGACY_PREFIX}{legacy}")] {
        let l = parse(&text).unwrap();
        assert_eq!(l.version, VERSION);
        assert_eq!(l.server.id, "rs.legacy");
        assert_eq!(l.server.key, "k");
    }
}

#[test]
fn validation_rejects_bad_files() {
    assert!(parse(r#"{"server": {"id": "x"}}"#).is_err(), "version is mandatory");
    let mut p = parse(FILE).unwrap();
    p.version = VERSION + 1;
    assert!(validate(&p).unwrap_err().to_string().contains("newer"));
    let mut p = parse(FILE).unwrap();
    p.options.insert("custom-rendezvous-server".into(), "dup".into());
    assert!(validate(&p).unwrap_err().to_string().contains("both"));
    let mut p = parse(FILE).unwrap();
    p.server.api = "rs.test:21114".into();
    assert!(validate(&p).unwrap_err().to_string().contains("http"));
    let mut p = parse(FILE).unwrap();
    p.locked.push("image-quality".into());
    assert!(validate(&p).unwrap_err().to_string().contains("no value"));
    assert!(validate(&parse(FILE).unwrap()).is_ok());
}

#[test]
fn install_fills_the_tables_by_trust_level() {
    let keys = [
        "custom-rendezvous-server", "relay-server", "api-server", "key",
        "verification-method", "lang",
    ];
    clear(&keys);
    let p = parse(FILE).unwrap();

    let r = install(&p, Source::Untrusted);
    assert_eq!(r.locked, Vec::<String>::new());
    assert!(r.warnings.iter().any(|w| w.contains("locked keys ignored")));
    assert!(r.ignored.contains(&"permanent-password".to_string()));
    assert!(r.ignored.contains(&"no-such-key".to_string()));
    assert_eq!(config::DEFAULT_SETTINGS.read().unwrap()["custom-rendezvous-server"], "rs.test");
    assert!(config::OVERWRITE_SETTINGS.read().unwrap().get("custom-rendezvous-server").is_none());
    clear(&keys);

    let r = install(&p, Source::File);
    assert_eq!(r.locked, vec!["custom-rendezvous-server", "verification-method"]);
    assert!(r.applied.contains(&"relay-server".to_string()));
    assert!(r.applied.contains(&"lang".to_string()));
    assert_eq!(r.secrets, vec!["permanent-password"]);
    assert_eq!(r.ignored, vec!["no-such-key"]);
    assert_eq!(config::OVERWRITE_SETTINGS.read().unwrap()["custom-rendezvous-server"], "rs.test");
    assert_eq!(config::DEFAULT_SETTINGS.read().unwrap()["relay-server"], "rs.test");
    assert!(config::DEFAULT_SETTINGS.read().unwrap().get("permanent-password").is_none());
    assert_eq!(config::DEFAULT_LOCAL_SETTINGS.read().unwrap()["lang"], "zh-CN");
    // a locked key wins over whatever the user has, an unlocked default yields to a user value
    assert_eq!(config::Config::get_option("custom-rendezvous-server"), "rs.test");
    clear(&keys);
}

#[test]
fn share_payload_is_capped_and_sanitized() {
    let mut p = parse(FILE).unwrap();
    let s = sanitized(&p);
    assert!(!s.options.contains_key("permanent-password"));
    assert_eq!(s.locked, p.locked, "the stored copy keeps the locks");
    p.options.insert("image-quality".into(), "x".repeat(1500));
    assert!(encode_share(&p).unwrap_err().to_string().contains("too large"));
    assert!(is_secret_key("permanent-password") && is_secret_key("unlock-pin") && is_secret_key("api-token"));
    assert!(!is_secret_key("key") && !is_secret_key("api-server"));
}
