use serde::Deserialize;
use std::env;

pub fn get_host_ip() -> String {
    env::var("HOST_IP").unwrap_or_else(|_| "0.0.0.0".to_string())
}

pub fn get_host_port() -> String {
    env::var("HOST_PORT").unwrap_or_else(|_| "8080".to_string())
}

pub fn get_host_domain() -> String {
    env::var("HOST_DOMAIN").unwrap_or_else(|_| format!("{}:{}", get_host_ip(), get_host_port()))
}

pub fn get_app_settings() -> anyhow::Result<AppSettings> {
    let config_file = env::var("CONFIG").unwrap_or_else(|_| "Storage.json".to_string());
    let config = std::fs::read_to_string(config_file)?;
    Ok(serde_json::from_str(&config)?)
}

#[derive(Deserialize)]
pub struct AppSettings {
    pub storage_root: String,
}
