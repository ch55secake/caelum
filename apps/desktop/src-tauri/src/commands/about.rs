use caelum_core::updates;
use caelum_protocol::{AppInfo, UpdateCheck};

#[tauri::command]
pub(crate) fn app_info() -> AppInfo {
    AppInfo {
        name: "Caelum".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        repository_url: updates::REPOSITORY_URL.to_owned(),
        releases_url: updates::RELEASES_URL.to_owned(),
        license: "MIT".to_owned(),
        license_url: updates::LICENSE_URL.to_owned(),
    }
}

#[tauri::command]
pub(crate) async fn check_for_updates() -> Result<UpdateCheck, String> {
    tauri::async_runtime::spawn_blocking(|| updates::check_for_updates(env!("CARGO_PKG_VERSION")))
        .await
        .map_err(|error| error.to_string())?
}
