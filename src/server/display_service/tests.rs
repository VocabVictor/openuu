use super::normalize_primary_display_idx;

#[test]
fn normalize_primary_display_idx_bounds() {
    assert_eq!(normalize_primary_display_idx(0, 0), 0);
    assert_eq!(normalize_primary_display_idx(0, 2), 0);
    assert_eq!(normalize_primary_display_idx(1, 2), 1);
    assert_eq!(normalize_primary_display_idx(2, 2), 0);
}
