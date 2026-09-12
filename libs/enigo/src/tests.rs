use super::*;
#[test]
fn test_get_key_state() {
    let mut enigo = Enigo::new();
    let keys = [Key::CapsLock, Key::NumLock];
    for k in keys.iter() {
        enigo.key_click(k.clone());
        let a = enigo.get_key_state(k.clone());
        enigo.key_click(k.clone());
        let b = enigo.get_key_state(k.clone());
        assert!(a != b);
    }
    let keys = [Key::Control, Key::Alt, Key::Shift];
    for k in keys.iter() {
        enigo.key_down(k.clone()).ok();
        let a = enigo.get_key_state(k.clone());
        enigo.key_up(k.clone());
        let b = enigo.get_key_state(k.clone());
        assert!(a != b);
    }
}
