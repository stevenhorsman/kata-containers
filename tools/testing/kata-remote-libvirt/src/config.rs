use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the Unix socket for the ttrpc server
    #[serde(default = "default_socket_path")]
    pub socket_path: PathBuf,

    /// Libvirt connection URI
    #[serde(default = "default_libvirt_uri")]
    pub libvirt_uri: String,

    /// Base image volume in format "pool/volume"
    #[serde(default = "default_base_volume")]
    pub base_volume: String,

    /// Default number of vCPUs
    #[serde(default = "default_vcpus")]
    pub default_vcpus: u32,

    /// Default memory in MiB
    #[serde(default = "default_memory")]
    pub default_memory: u64,
}

fn default_socket_path() -> PathBuf {
    PathBuf::from("/run/kata-remote-libvirt/hypervisor.sock")
}

fn default_libvirt_uri() -> String {
    "qemu:///system".to_string()
}

fn default_base_volume() -> String {
    "default/kata-base.qcow2".to_string()
}

fn default_vcpus() -> u32 {
    1
}

fn default_memory() -> u64 {
    2048
}

impl Default for Config {
    fn default() -> Self {
        Self {
            socket_path: default_socket_path(),
            libvirt_uri: default_libvirt_uri(),
            base_volume: default_base_volume(),
            default_vcpus: default_vcpus(),
            default_memory: default_memory(),
        }
    }
}

impl Config {
    pub fn from_file(path: &PathBuf) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }
}
