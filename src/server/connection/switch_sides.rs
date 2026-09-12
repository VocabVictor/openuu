use super::*;

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn insert_switch_sides_uuid(id: String, uuid: uuid::Uuid) {
    SWITCH_SIDES_UUID
        .lock()
        .unwrap()
        .insert(id, (tokio::time::Instant::now(), uuid));
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn insert_pending_switch_sides_uuid(id: String, uuid: uuid::Uuid) -> bool {
    let mut uuids = PENDING_SWITCH_SIDES_UUID.lock().unwrap();
    uuids.retain(|_, (instant, _, _)| instant.elapsed() < SWITCH_SIDES_UUID_TTL);
    if uuids.get(&id).map(|(_, stored_uuid, _)| stored_uuid) == Some(&uuid) {
        return false;
    }
    uuids.insert(id, (tokio::time::Instant::now(), uuid, false));
    true
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn has_pending_switch_sides_uuid(id: &str, uuid: &uuid::Uuid) -> bool {
    let mut uuids = PENDING_SWITCH_SIDES_UUID.lock().unwrap();
    uuids.retain(|_, (instant, _, _)| instant.elapsed() < SWITCH_SIDES_UUID_TTL);
    uuids
        .get(id)
        .map(|(_, stored_uuid, claimed)| stored_uuid == uuid && !*claimed)
        == Some(true)
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn claim_pending_switch_sides_uuid(id: &str, uuid: &uuid::Uuid) -> bool {
    let mut uuids = PENDING_SWITCH_SIDES_UUID.lock().unwrap();
    uuids.retain(|_, (instant, _, _)| instant.elapsed() < SWITCH_SIDES_UUID_TTL);
    // Keep claimed entries until expiry so replaying a request cannot launch another connection.
    if let Some((_, stored_uuid, claimed)) = uuids.get_mut(id) {
        if stored_uuid == uuid && !*claimed {
            *claimed = true;
            return true;
        }
    }
    false
}
