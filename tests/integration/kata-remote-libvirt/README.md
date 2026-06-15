# Kata Remote Libvirt Integration Tests

Integration tests for the kata-remote-libvirt hypervisor implementation.

## Overview

These tests verify that the kata-remote hypervisor works correctly with libvirt as the VM backend. The tests are designed to run in CI and locally for development.

## Test Structure

- `gha-run.sh` - Main test orchestration script
- Smoke tests that verify:
  - Server starts and socket is created
  - Libvirt connection works
  - Base image exists
  - Server logs are clean

## Prerequisites

### System Requirements

- Ubuntu 24.04 or later
- Nested virtualization support (for running VMs)
- At least 4GB RAM
- 20GB free disk space

### Software Dependencies

The test script will install:
- build-essential
- libvirt-daemon-system
- libvirt-dev
- pkg-config (required for Rust virt crate)
- qemu-kvm
- qemu-utils
- virtinst
- virt-manager
- bridge-utils
- jq
- Rust toolchain

## Running Tests Locally

### Full Test Run

```bash
# Install dependencies
./tests/integration/kata-remote-libvirt/gha-run.sh install-dependencies

# Build kata-remote-libvirt binary
./tests/integration/kata-remote-libvirt/gha-run.sh build-binary

# Create base image (placeholder for now)
./tests/integration/kata-remote-libvirt/gha-run.sh create-base-image

# Install kata from tarball (if you have one)
./tests/integration/kata-remote-libvirt/gha-run.sh install-kata kata-artifacts

# Configure kata runtime
./tests/integration/kata-remote-libvirt/gha-run.sh configure-runtime

# Run tests
./tests/integration/kata-remote-libvirt/gha-run.sh run

# Cleanup
./tests/integration/kata-remote-libvirt/gha-run.sh cleanup
```

### Individual Steps

You can run individual steps for debugging:

```bash
# Just install dependencies
./tests/integration/kata-remote-libvirt/gha-run.sh install-dependencies

# Just build the binary
./tests/integration/kata-remote-libvirt/gha-run.sh build-binary

# Just run tests (assumes everything else is set up)
./tests/integration/kata-remote-libvirt/gha-run.sh run
```

## CI Integration

The tests are integrated into the main CI workflow via `.github/workflows/run-kata-remote-tests.yaml`.

### Workflow Inputs

- `tarball-suffix` - Suffix for the kata tarball artifact
- `commit-hash` - Git commit to test
- `target-branch` - Target branch for rebasing

### Workflow Steps

1. Checkout code
2. Install dependencies
3. Download kata tarball
4. Install kata
5. Build kata-remote-libvirt
6. Create base image
7. Configure runtime
8. Run tests
9. Cleanup

## Configuration

### Environment Variables

- `LIBVIRT_POOL` - Libvirt storage pool name (default: `default`)
- `LIBVIRT_BASE_IMAGE` - Base image name (default: `kata-base.qcow2`)
- `KATA_REMOTE_SOCKET` - Socket path (default: `/run/kata-remote-libvirt/hypervisor.sock`)
- `KATA_REMOTE_BINARY` - Binary name (default: `kata-remote-libvirt`)
- `KATA_HYPERVISOR` - Hypervisor type (should be `remote`)

### Kata Configuration

The tests use a dropin configuration file to override the remote hypervisor socket:

```toml
# /opt/kata/share/defaults/kata-containers/config.d/50-kata-remote-libvirt.toml
[hypervisor.remote]
remote_hypervisor_socket = "/run/kata-remote-libvirt/hypervisor.sock"
```

## Base Image

Currently, the `create-base-image` step creates a placeholder image. For full functionality, you need a proper base image with:

1. Minimal Linux OS (Alpine, Ubuntu, etc.)
2. kata-agent installed and configured
3. kata-agent starts on boot
4. vsock kernel support enabled

See `tools/testing/kata-remote-libvirt/README.md` for details on creating a proper base image.

## Troubleshooting

### Server fails to start

Check the server logs:
```bash
cat /tmp/kata-remote-libvirt.log
```

### Libvirt connection fails

Verify libvirt is running:
```bash
sudo systemctl status libvirtd
sudo virsh list
```

### Base image not found

List available images:
```bash
sudo virsh vol-list default
```

### Permission denied on socket

Ensure you're in the libvirt group:
```bash
groups
sudo usermod -a -G libvirt $(whoami)
# Log out and back in for group changes to take effect
```

### Build fails with "pkg-config not found"

Make sure pkg-config is installed:
```bash
sudo apt-get update && sudo apt-get install -y libvirt-dev pkg-config
```

## Known Limitations

- Base image creation is currently a placeholder
- Tests are smoke tests only (no full container lifecycle)
- Requires nested virtualization support
- Only tested on x86_64 Ubuntu

## Future Enhancements

- [ ] Full base image creation with kata-agent
- [ ] Container lifecycle tests (create, start, stop, delete)
- [ ] Multi-container tests
- [ ] Network connectivity tests
- [ ] Volume mount tests
- [ ] Integration with containerd/crio
- [ ] Support for other architectures (arm64, s390x)
