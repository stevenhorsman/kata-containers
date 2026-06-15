use anyhow::Result;
use async_trait::async_trait;
use protocols::remote::{CreateVMRequest, CreateVMResponse, StartVMRequest, StartVMResponse,
                        StopVMRequest, StopVMResponse, VersionRequest, VersionResponse};
use protocols::remote_ttrpc_async::{create_hypervisor, Hypervisor};
use std::sync::Arc;
use ttrpc::r#async::Server;

use crate::libvirt::LibvirtManager;

#[derive(Clone)]
pub struct HypervisorService {
    libvirt: Arc<LibvirtManager>,
}

impl HypervisorService {
    pub fn new(libvirt: LibvirtManager) -> Self {
        Self {
            libvirt: Arc::new(libvirt),
        }
    }
}

#[async_trait]
impl Hypervisor for HypervisorService {
    async fn version(
        &self,
        _ctx: &ttrpc::r#async::TtrpcContext,
        _req: VersionRequest,
    ) -> ttrpc::Result<VersionResponse> {
        tracing::debug!("Version request received");

        let mut resp = VersionResponse::new();
        resp.version = "0.1.0".to_string();

        Ok(resp)
    }

    async fn create_vm(
        &self,
        _ctx: &ttrpc::r#async::TtrpcContext,
        req: CreateVMRequest,
    ) -> ttrpc::Result<CreateVMResponse> {
        tracing::info!("CreateVM request for VM: {}", req.id);

        let agent_socket = self.libvirt
            .create_vm(&req.id, &req.annotations)
            .map_err(|e| {
                tracing::error!("Failed to create VM: {}", e);
                ttrpc::Error::Others(format!("Failed to create VM: {}", e))
            })?;

        let mut resp = CreateVMResponse::new();
        resp.agentSocketPath = agent_socket;

        tracing::info!("CreateVM successful for VM: {}", req.id);
        Ok(resp)
    }

    async fn start_vm(
        &self,
        _ctx: &ttrpc::r#async::TtrpcContext,
        req: StartVMRequest,
    ) -> ttrpc::Result<StartVMResponse> {
        tracing::info!("StartVM request for VM: {}", req.id);

        self.libvirt
            .start_vm(&req.id)
            .map_err(|e| {
                tracing::error!("Failed to start VM: {}", e);
                ttrpc::Error::Others(format!("Failed to start VM: {}", e))
            })?;

        tracing::info!("StartVM successful for VM: {}", req.id);
        Ok(StartVMResponse::new())
    }

    async fn stop_vm(
        &self,
        _ctx: &ttrpc::r#async::TtrpcContext,
        req: StopVMRequest,
    ) -> ttrpc::Result<StopVMResponse> {
        tracing::info!("StopVM request for VM: {}", req.id);

        self.libvirt
            .stop_vm(&req.id)
            .map_err(|e| {
                tracing::error!("Failed to stop VM: {}", e);
                ttrpc::Error::Others(format!("Failed to stop VM: {}", e))
            })?;

        tracing::info!("StopVM successful for VM: {}", req.id);
        Ok(StopVMResponse::new())
    }
}

pub async fn start_server(socket_path: &str, libvirt: LibvirtManager) -> Result<()> {
    // Remove existing socket if it exists
    if std::path::Path::new(socket_path).exists() {
        std::fs::remove_file(socket_path)?;
    }

    // Create parent directory if it doesn't exist
    if let Some(parent) = std::path::Path::new(socket_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let service = HypervisorService::new(libvirt);
    let hypervisor_service = create_hypervisor(Arc::new(service));

    // ttrpc requires unix:// prefix for socket paths
    let socket_uri = format!("unix://{}", socket_path);

    let mut server = Server::new()
        .bind(&socket_uri)?
        .register_service(hypervisor_service);

    tracing::info!("ttrpc server listening on: {}", socket_path);

    server.start().await?;

    Ok(())
}
