# kata-remote-libvirt

A minimal remote hypervisor implementation for Kata Containers using libvirt. This is a testing tool that implements the kata-remote hypervisor protocol using libvirt to manage VMs.

## Purpose

This tool provides a simple way to test the kata-remote hypervisor functionality without requiring a full cloud infrastructure. It uses libvirt to create and manage VMs locally, making it ideal for development and testing.

## Prerequisites

- **Rust toolchain**
- **libvirt** installed and running
  ```bash
  # On Ubuntu/Debian
  sudo apt-get install libvirt-daemon-system libvirt-dev

  # On Fedora/RHEL
  sudo dnf install libvirt libvirt-devel
  ```
- **Kata Containers** installed
- **Base VM image** configured in libvirt
  - Create a storage pool (default: `default`)
  - Add a base image volume (default: `kata-base.qcow2`)
  - The image should have kata-agent installed and configured

## Building

```bash
cd tools/testing/kata-remote-libvirt
cargo build --release
```

The binary will be available at `target/release/kata-remote-libvirt`.

## Running

### Basic Usage

```bash
sudo ./target/release/kata-remote-libvirt
```

This will start the hypervisor with default settings:
- Socket: `/run/kata-remote-libvirt/hypervisor.sock`
- Libvirt URI: `qemu:///system`
- Base volume: `default/kata-base.qcow2`

### Command-Line Options

```bash
kata-remote-libvirt [OPTIONS]

Options:
  -s, --socket <SOCKET>              Path to the Unix socket for the ttrpc server
  -c, --config <CONFIG>              Path to configuration file
  -l, --libvirt-uri <LIBVIRT_URI>    Libvirt connection URI
  -b, --base-volume <BASE_VOLUME>    Base volume in format "pool/volume"
      --log-level <LOG_LEVEL>        Log level (trace, debug, info, warn, error) [default: info]
  -h, --help                         Print help
```

### Configuration File

Create a TOML configuration file:

```toml
socket_path = "/run/kata-remote-libvirt/hypervisor.sock"
libvirt_uri = "qemu:///system"
base_volume = "default/kata-base.qcow2"
default_vcpus = 1
default_memory = 2048
```

Then run with:

```bash
sudo ./target/release/kata-remote-libvirt --config /path/to/config.toml
```

## Configuring Kata Runtime

To use this hypervisor with Kata Containers, configure the runtime to use the remote hypervisor:

1. Edit the Kata configuration file (e.g., `/etc/kata-containers/configuration.toml`):

```toml
[hypervisor.remote]
remote_hypervisor_socket = "/run/kata-remote-libvirt/hypervisor.sock"
remote_hypervisor_timeout = 60
```

2. Set the hypervisor type to `remote`:

```bash
sudo kata-runtime --kata-config /etc/kata-containers/configuration.toml
```

Or use containerd configuration to specify the remote hypervisor.

## Preparing the Base Image

The base image should be a VM image with:

1. **Kata agent** installed and configured to start on boot
2. **vsock support** enabled in the kernel
3. **Minimal OS** (Alpine, Ubuntu, etc.)

Example steps to create a base image:

```bash
# Create a qcow2 image
qemu-img create -f qcow2 kata-base.qcow2 10G

# Install OS and kata-agent (details depend on your distribution)
# ...

# Add to libvirt storage pool
virsh pool-list
virsh vol-create-as default kata-base.qcow2 10G --format qcow2
virsh vol-upload --pool default kata-base.qcow2 /path/to/kata-base.qcow2
```

## Testing

### Basic Test

1. Start the hypervisor:
   ```bash
   sudo ./target/release/kata-remote-libvirt --log-level debug
   ```

2. In another terminal, test with a Kata container:
   ```bash
   sudo ctr run --runtime io.containerd.kata.v2 --rm docker.io/library/busybox:latest test-container sh
   ```

### Manual Testing with ttrpc

You can also test the hypervisor directly using the ttrpc protocol (requires a ttrpc client tool).

## Architecture

```
┌─────────────────────┐
│  Kata Runtime       │
│  (containerd/crio)  │
└──────────┬──────────┘
           │ ttrpc
           ▼
┌─────────────────────┐
│ kata-remote-libvirt │
│  - ttrpc server     │
│  - VM management    │
└──────────┬──────────┘
           │ libvirt API
           ▼
┌─────────────────────┐
│     libvirt         │
│  (qemu/kvm)         │
└─────────────────────┘
```

## Limitations

This is a **testing tool** and has several limitations:

- No production-grade error handling
- Limited VM lifecycle management
- No support for advanced features (snapshots, migration, etc.)
- Simple XML-based vsock CID extraction
- No resource cleanup on crashes
- Single-threaded libvirt operations

## Troubleshooting

### Socket permission denied

Make sure you run with `sudo` or have appropriate permissions for the socket directory.

### Libvirt connection failed

Check that libvirt is running:
```bash
sudo systemctl status libvirtd
```

### VM creation failed

- Verify the base volume exists: `virsh vol-list default`
- Check libvirt logs: `sudo journalctl -u libvirtd`
- Ensure the default network is active: `virsh net-list`

### No vsock CID found

The VM XML must include a vsock device. Check the domain XML after creation:
```bash
virsh dumpxml kata-remote-<vm-id>
```
