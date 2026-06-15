use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use virt::connect::Connect;
use virt::domain::Domain;

use crate::config::Config;

pub struct LibvirtManager {
    conn: Connect,
    config: Config,
}

impl LibvirtManager {
    pub fn new(config: Config) -> Result<Self> {
        let conn = Connect::open(Some(&config.libvirt_uri))
            .context(format!("Failed to connect to libvirt at {}", config.libvirt_uri))?;

        tracing::info!("Connected to libvirt: {}", config.libvirt_uri);

        Ok(Self { conn, config })
    }

    pub fn create_vm(
        &self,
        vm_id: &str,
        annotations: &HashMap<String, String>,
    ) -> Result<String> {
        tracing::info!("Creating VM: {}", vm_id);

        // Parse CPU and memory from annotations
        let vcpus = annotations
            .get("io.katacontainers.config.hypervisor.default_vcpus")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(self.config.default_vcpus);

        let memory = annotations
            .get("io.katacontainers.config.hypervisor.default_memory")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(self.config.default_memory);

        tracing::debug!("VM config - vcpus: {}, memory: {} MiB", vcpus, memory);

        // Parse base volume (format: "pool/volume")
        let parts: Vec<&str> = self.config.base_volume.split('/').collect();
        if parts.len() != 2 {
            return Err(anyhow!(
                "Invalid base_volume format: {}. Expected 'pool/volume'",
                self.config.base_volume
            ));
        }
        let (pool, volume) = (parts[0], parts[1]);

        // Create domain XML
        let domain_name = format!("kata-remote-{}", vm_id);
        let xml = format!(
            r#"<domain type='kvm'>
    <name>{}</name>
    <memory unit='MiB'>{}</memory>
    <vcpu>{}</vcpu>
    <os>
        <type arch='x86_64'>hvm</type>
    </os>
    <devices>
        <disk type='volume'>
            <source pool='{}' volume='{}'/>
            <target dev='vda' bus='virtio'/>
        </disk>
        <interface type='network'>
            <source network='default'/>
            <model type='virtio'/>
        </interface>
        <vsock model='virtio'>
            <cid auto='yes'/>
        </vsock>
        <serial type='pty'>
            <target type='system-serial' port='0'/>
        </serial>
        <console type='pty'>
            <target type='serial' port='0'/>
        </console>
    </devices>
</domain>"#,
            domain_name, memory, vcpus, pool, volume
        );

        tracing::debug!("Domain XML:\n{}", xml);

        // Define the domain
        let domain = Domain::define_xml(&self.conn, &xml)
            .context("Failed to define domain")?;

        tracing::info!("Domain defined: {}", domain_name);

        // Get the vsock CID
        let cid = self.get_vsock_cid(&domain)?;
        let agent_socket = format!("vsock://{}:1024", cid);

        tracing::info!("VM created with vsock CID: {}", cid);

        Ok(agent_socket)
    }

    pub fn start_vm(&self, vm_id: &str) -> Result<()> {
        tracing::info!("Starting VM: {}", vm_id);

        let domain_name = format!("kata-remote-{}", vm_id);
        let domain = Domain::lookup_by_name(&self.conn, &domain_name)
            .context(format!("Failed to lookup domain: {}", domain_name))?;

        domain.create().context("Failed to start domain")?;

        tracing::info!("VM started: {}", vm_id);
        Ok(())
    }

    pub fn stop_vm(&self, vm_id: &str) -> Result<()> {
        tracing::info!("Stopping VM: {}", vm_id);

        let domain_name = format!("kata-remote-{}", vm_id);
        let domain = Domain::lookup_by_name(&self.conn, &domain_name)
            .context(format!("Failed to lookup domain: {}", domain_name))?;

        // Force stop (destroy)
        if let Err(e) = domain.destroy() {
            tracing::warn!("Failed to destroy domain (may already be stopped): {}", e);
        }

        // Undefine the domain
        domain.undefine().context("Failed to undefine domain")?;

        tracing::info!("VM stopped and undefined: {}", vm_id);
        Ok(())
    }

    fn get_vsock_cid(&self, domain: &Domain) -> Result<u32> {
        // Get domain XML to extract vsock CID
        let xml = domain.get_xml_desc(0).context("Failed to get domain XML")?;

        // Parse XML to find vsock CID
        // This is a simple string search - in production, use proper XML parsing
        if let Some(start) = xml.find("<cid address='") {
            let start = start + "<cid address='".len();
            if let Some(end) = xml[start..].find('\'') {
                let cid_str = &xml[start..start + end];
                return cid_str.parse::<u32>()
                    .context(format!("Failed to parse vsock CID: {}", cid_str));
            }
        }

        Err(anyhow!("Failed to find vsock CID in domain XML"))
    }
}
