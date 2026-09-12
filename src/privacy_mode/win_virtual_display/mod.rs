use super::{PrivacyMode, PrivacyModeState, INVALID_PRIVACY_MODE_CONN_ID, NO_PHYSICAL_DISPLAYS};
use crate::{platform::windows::reg_display_settings, virtual_display_manager};
use hbb_common::{allow_err, bail, config::Config, log, ResultType};
use std::{
    io::Error,
    ops::{Deref, DerefMut},
    thread,
    time::Duration,
};
use virtual_display::MonitorMode;
use winapi::{
    shared::{
        minwindef::{DWORD, FALSE},
        ntdef::{NULL, WCHAR},
    },
    um::{
        wingdi::{
            DEVMODEW, DISPLAY_DEVICEW, DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP,
            DISPLAY_DEVICE_MIRRORING_DRIVER, DISPLAY_DEVICE_PRIMARY_DEVICE, DM_POSITION,
        },
        winuser::{
            ChangeDisplaySettingsExW, EnumDisplayDevicesW, EnumDisplaySettingsExW,
            EnumDisplaySettingsW, CDS_NORESET, CDS_RESET, CDS_SET_PRIMARY, CDS_UPDATEREGISTRY,
            DISP_CHANGE_FAILED, DISP_CHANGE_SUCCESSFUL, EDD_GET_DEVICE_INTERFACE_NAME,
            ENUM_CURRENT_SETTINGS, ENUM_REGISTRY_SETTINGS,
        },
    },
};

pub(super) const PRIVACY_MODE_IMPL: &str = super::PRIVACY_MODE_IMPL_WIN_VIRTUAL_DISPLAY;

mod displays;
mod ensure;

const CONFIG_KEY_REG_RECOVERY: &str = "reg_recovery";

struct Display {
    dm: DEVMODEW,
    name: [WCHAR; 32],
    primary: bool,
}

pub struct PrivacyModeImpl {
    impl_key: String,
    conn_id: i32,
    displays: Vec<Display>,
    virtual_displays: Vec<Display>,
    virtual_displays_added: Vec<u32>,
}

struct TurnOnGuard<'a> {
    privacy_mode: &'a mut PrivacyModeImpl,
    succeeded: bool,
}

impl<'a> Deref for TurnOnGuard<'a> {
    type Target = PrivacyModeImpl;

    fn deref(&self) -> &Self::Target {
        self.privacy_mode
    }
}

impl<'a> DerefMut for TurnOnGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.privacy_mode
    }
}

impl<'a> Drop for TurnOnGuard<'a> {
    fn drop(&mut self) {
        if !self.succeeded {
            self.privacy_mode
                .turn_off_privacy(INVALID_PRIVACY_MODE_CONN_ID, None)
                .ok();
        }
    }
}

impl PrivacyMode for PrivacyModeImpl {
    fn is_async_privacy_mode(&self) -> bool {
        virtual_display_manager::is_amyuni_idd()
    }

    fn init(&self) -> ResultType<()> {
        Ok(())
    }

    fn clear(&mut self) {
        allow_err!(self.turn_off_privacy(self.conn_id, None));
    }

    fn turn_on_privacy(&mut self, conn_id: i32) -> ResultType<bool> {
        if !virtual_display_manager::is_virtual_display_supported() {
            bail!("idd_not_support_under_win10_2004_tip");
        }

        if self.check_on_conn_id(conn_id)? {
            log::debug!("Privacy mode of conn {} is already on", conn_id);
            return Ok(true);
        }
        self.set_displays();
        if self.displays.is_empty() {
            log::debug!("{}", NO_PHYSICAL_DISPLAYS);
            bail!(NO_PHYSICAL_DISPLAYS);
        }

        let is_async_mode = self.is_async_privacy_mode();
        let mut guard = TurnOnGuard {
            privacy_mode: self,
            succeeded: false,
        };

        guard.ensure_virtual_display(is_async_mode)?;
        if guard.virtual_displays.is_empty() {
            log::debug!("No virtual displays");
            bail!("No virtual displays.");
        }

        let reg_connectivity_1 = reg_display_settings::read_reg_connectivity()?;
        let primary_display_name = guard.set_primary_display()?;
        guard.disable_physical_displays()?;
        Self::commit_change_display(CDS_RESET)?;
        // Explicitly set the resolution(virtual display) to 1920x1080.
        allow_err!(crate::platform::change_resolution(
            &primary_display_name,
            1920,
            1080
        ));
        let reg_connectivity_2 = reg_display_settings::read_reg_connectivity()?;

        if let Some(reg_recovery) =
            reg_display_settings::diff_recent_connectivity(reg_connectivity_1, reg_connectivity_2)
        {
            Config::set_option(
                CONFIG_KEY_REG_RECOVERY.to_owned(),
                serde_json::to_string(&reg_recovery)?,
            );
        } else {
            reset_config_reg_connectivity();
        };

        // OpenInputDesktop and block the others' input ?
        guard.conn_id = conn_id;
        guard.succeeded = true;

        allow_err!(super::win_input::hook());

        Ok(true)
    }

    fn turn_off_privacy(
        &mut self,
        conn_id: i32,
        state: Option<PrivacyModeState>,
    ) -> ResultType<()> {
        self.check_off_conn_id(conn_id)?;
        super::win_input::unhook()?;
        let _tmp_ignore_changed_holder = crate::display_service::temp_ignore_displays_changed();
        self.restore();
        // We need to force restore the registry connectivity.
        // This is because the registry connection may be changed by `self.restore()`, but will not be fully restored.
        restore_reg_connectivity(false, true);

        if self.conn_id != INVALID_PRIVACY_MODE_CONN_ID {
            if let Some(state) = state {
                allow_err!(super::set_privacy_mode_state(
                    conn_id,
                    state,
                    PRIVACY_MODE_IMPL.to_string(),
                    1_000
                ));
            }
            self.conn_id = INVALID_PRIVACY_MODE_CONN_ID.to_owned();
        }

        Ok(())
    }

    #[inline]
    fn pre_conn_id(&self) -> i32 {
        self.conn_id
    }

    #[inline]
    fn get_impl_key(&self) -> &str {
        &self.impl_key
    }
}

impl Drop for PrivacyModeImpl {
    fn drop(&mut self) {
        if self.conn_id != INVALID_PRIVACY_MODE_CONN_ID {
            allow_err!(self.turn_off_privacy(self.conn_id, None));
        }
    }
}

#[inline]
fn reset_config_reg_connectivity() {
    Config::set_option(CONFIG_KEY_REG_RECOVERY.to_owned(), "".to_owned());
}

pub fn restore_reg_connectivity(plug_out_monitors: bool, force: bool) {
    let config_recovery_value = Config::get_option(CONFIG_KEY_REG_RECOVERY);
    if config_recovery_value.is_empty() {
        return;
    }
    if plug_out_monitors {
        let _ = virtual_display_manager::plug_out_monitor(-1, true, false);
    }
    if let Ok(reg_recovery) =
        serde_json::from_str::<reg_display_settings::RegRecovery>(&config_recovery_value)
    {
        if let Err(e) = reg_display_settings::restore_reg_connectivity(reg_recovery, force) {
            log::error!("Failed restore_reg_connectivity, error: {}", e);
        }
    }
    reset_config_reg_connectivity();
}
