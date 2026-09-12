use hbb_common::{anyhow::{anyhow, bail}, ResultType};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::Command;

mod catalog;
use catalog::*;
mod launch;
use launch::*;
#[cfg(test)]
mod tests;
mod icons;
use icons::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct App {
    pub id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub request_id: String,
    pub operation: String,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub custom: bool,
}

pub fn handle(raw: &str) -> String {
    let mut request_id = String::new();
    let result = (|| -> ResultType<serde_json::Value> {
        if raw.len() > 32768 { bail!("Request is too large"); }
        let request: Request = serde_json::from_str(raw)?;
        request_id = request.request_id.clone();
        if request_id.len() > 128 || request.arguments.len() > 64 ||
            request.arguments.iter().any(|s| s.contains('\0') || s.len() > 4096) {
            bail!("Invalid request");
        }
        let (session, user, home) = desktop()?;
        if !request.user.is_empty() && request.user != user {
            bail!("The shortcut belongs to another desktop user");
        }
        if !request.session.is_empty() && request.session != session {
            bail!("Switch the remote desktop to the requested user session first");
        }
        match request.operation.as_str() {
            "list" => Ok(serde_json::json!({"apps": catalog(&home), "session": session, "user": user})),
            "launch" | "icon" => {
                let path = PathBuf::from(&request.app_id);
                if !path.is_absolute() || !path.exists() { bail!("Application was removed or the path is invalid"); }
                let app = if request.custom {
                    App { id: request.app_id.clone(), name: request.app_id.clone(), kind: "executable".into() }
                } else {
                    catalog(&home).into_iter().find(|a| a.id == request.app_id)
                        .ok_or_else(|| anyhow!("Application is no longer in the desktop catalog"))?
                };
                if request.operation == "icon" {
                    let icon = app_icon(&app).map(|bytes| hbb_common::base64::encode(bytes)).unwrap_or_default();
                    return Ok(serde_json::json!({"icon": icon}));
                }
                launch(&app, &request.arguments, &session)?;
                Ok(serde_json::json!({"accepted": true, "session": session}))
            }
            _ => bail!("Unsupported operation"),
        }
    })();
    match result {
        Ok(data) => serde_json::json!({"request_id": request_id, "data": data}).to_string(),
        Err(e) => serde_json::json!({"request_id": request_id, "error": e.to_string()}).to_string(),
    }
}

pub fn denied(raw: &str) -> String {
    let id = if raw.len() <= 32768 {
        serde_json::from_str::<Request>(raw).ok().map(|r| r.request_id).filter(|id| id.len() <= 128).unwrap_or_default()
    } else { String::new() };
    serde_json::json!({"request_id": id, "error": "Quick launch requires an authorized control session"}).to_string()
}
