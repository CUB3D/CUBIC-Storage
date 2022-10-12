use anyhow::Context;
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

pub struct AppSettings {
    pub storage_root: String,
}

impl AppSettings {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            storage_root: env::var("STORAGE_ROOT").context("No storage root specified")?,
        })
    }
}
