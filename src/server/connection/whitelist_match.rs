use super::*;

// An empty whitelist allows everyone.
//
// A peer connecting across servers reports `<its id>@<its own server>` (see
// `create_login_msg`), so the bare id is matched as well. That suffix is self-asserted and
// unsigned, so matching only the full form would reject the honest cross-server peer while
// an attacker just reports the bare id: it can produce false rejects but no true ones.
pub(super) fn id_whitelist_allows(id_whitelist: &[String], my_id: &str) -> bool {
    if id_whitelist.is_empty() {
        return true;
    }
    let bare_id = my_id.split('@').next().unwrap_or(my_id);
    id_whitelist
        .iter()
        .any(|x| wildcard_match(x, my_id) || wildcard_match(x, bare_id))
}

// Drop `keys` whose last failure (`.0`, in minutes) is at least `window` old. A backwards
// clock gives a negative age and keeps the entry, so it never widens access.
pub(super) fn decay_stale_failures(
    failures: &mut HashMap<String, (i32, i32, i32)>,
    keys: &[String],
    now: i32,
    window: i32,
) {
    for key in keys {
        if failures
            .get(key)
            .is_some_and(|v| now.saturating_sub(v.0) >= window)
        {
            failures.remove(key);
        }
    }
}

// Unconditionally forget `keys`, unlike `update_failure`'s remove path which requires the
// per-address entry to exist.
pub(super) fn clear_failures(failures: &mut HashMap<String, (i32, i32, i32)>, keys: &[String]) {
    for key in keys {
        failures.remove(key);
    }
}

// Simple glob matching for the ID whitelist: '*' matches any sequence of characters
// (including the empty one), '?' matches exactly one character. Case-insensitive.
pub(super) fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.trim().to_lowercase().chars().collect();
    let t: Vec<char> = text.trim().to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some((pi + 1, ti));
            pi += 1;
        } else if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if let Some((sp, st)) = star {
            pi = sp;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}
