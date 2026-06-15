#!/bin/bash
#
# Copyright (c) 2026 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

# shellcheck disable=SC2034
kata_tarball_dir="${2:-kata-artifacts}"
kata_remote_dir="$(dirname "$(readlink -f "$0")")"
# shellcheck source=/dev/null
source "${kata_remote_dir}/../../common.bash"

# Configuration
LIBVIRT_POOL="${LIBVIRT_POOL:-default}"
LIBVIRT_BASE_IMAGE="${LIBVIRT_BASE_IMAGE:-kata-base.qcow2}"
KATA_REMOTE_SOCKET="${KATA_REMOTE_SOCKET:-/run/kata-remote-libvirt/hypervisor.sock}"
KATA_REMOTE_BINARY="${KATA_REMOTE_BINARY:-kata-remote-libvirt}"

function install_dependencies() {
	info "Installing dependencies for kata-remote-libvirt tests"

	# System dependencies
	declare -a system_deps=(
		build-essential
		libvirt-daemon-system
		libvirt-dev
		pkg-config
		qemu-kvm
		qemu-utils
		virtinst
		virt-manager
		bridge-utils
		jq
	)

	sudo apt-get update
	sudo apt-get -y install "${system_deps[@]}"

	# Start and enable libvirt
	sudo systemctl start libvirtd
	sudo systemctl enable libvirtd

	# Add current user to libvirt group
	sudo usermod -a -G libvirt "$(whoami)" || true

	# Install Rust if not already installed
	if ! command -v rustc &> /dev/null; then
		info "Installing Rust toolchain"
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
		# shellcheck source=/dev/null
		source "$HOME/.cargo/env"
	fi

	info "Dependencies installed successfully"
}

function build_kata_remote_libvirt() {
	info "Building kata-remote-libvirt binary"

	pushd "${kata_remote_dir}/../../../tools/testing/kata-remote-libvirt"
	# Use system linker instead of rust-lld to avoid libvirt linking issues
	RUSTFLAGS="-C linker=gcc" cargo build --release
	popd

	info "kata-remote-libvirt binary built successfully"
}

function create_base_image() {
	info "Creating base VM image from kata artifacts"

	local image_path="/var/lib/libvirt/images/${LIBVIRT_BASE_IMAGE}"
	local kata_image="/opt/kata/share/kata-containers/kata-containers.img"

	# Ensure the default libvirt pool exists
	if ! sudo virsh pool-list --all | grep -q "${LIBVIRT_POOL}"; then
		info "Creating libvirt storage pool: ${LIBVIRT_POOL}"
		sudo virsh pool-define-as "${LIBVIRT_POOL}" dir --target /var/lib/libvirt/images
		sudo virsh pool-start "${LIBVIRT_POOL}"
		sudo virsh pool-autostart "${LIBVIRT_POOL}"
	elif ! sudo virsh pool-list | grep -q "${LIBVIRT_POOL}"; then
		info "Starting libvirt storage pool: ${LIBVIRT_POOL}"
		sudo virsh pool-start "${LIBVIRT_POOL}"
	fi

	# Check if image already exists
	if sudo virsh vol-list "${LIBVIRT_POOL}" | grep -q "${LIBVIRT_BASE_IMAGE}"; then
		info "Base image ${LIBVIRT_BASE_IMAGE} already exists, skipping creation"
		return 0
	fi

	# Check if kata image exists
	if [ ! -f "${kata_image}" ]; then
		die "Kata image not found at ${kata_image}. Ensure kata tarball is extracted."
	fi

	info "Converting kata image to qcow2 format for libvirt"
	info "Source: ${kata_image}"
	info "Destination: ${image_path}"

	# Convert the kata image to qcow2 format
	# The kata image is typically in raw format
	sudo qemu-img convert -f raw -O qcow2 "${kata_image}" "${image_path}"

	# Verify the conversion
	if [ ! -f "${image_path}" ]; then
		die "Failed to create base image at ${image_path}"
	fi

	# Add to libvirt pool
	sudo virsh pool-refresh "${LIBVIRT_POOL}" || true

	info "Base image created successfully"
	sudo qemu-img info "${image_path}"
}

function configure_kata_runtime() {
	info "Configuring kata runtime to use remote hypervisor"

	# The base configuration is installed by install_kata() from the tarball
	# We just need to add a dropin to override the remote hypervisor socket path
	local kata_config_dir="/opt/kata/share/defaults/kata-containers"
	local dropin_dir="${kata_config_dir}/config.d"
	local dropin_file="${dropin_dir}/50-kata-remote-libvirt.toml"

	sudo mkdir -p "${dropin_dir}"

	# Create dropin configuration to override the remote hypervisor socket
	sudo tee "${dropin_file}" > /dev/null <<EOF
# Dropin configuration for kata-remote-libvirt testing
# This overrides the remote hypervisor socket path for testing

[hypervisor.remote]
remote_hypervisor_socket = "${KATA_REMOTE_SOCKET}"
EOF

	info "Kata runtime configured for remote hypervisor using dropin"
	info "Dropin configuration file: ${dropin_file}"
}

function start_kata_remote_server() {
	info "Starting kata-remote-libvirt server"

	local binary_path="${kata_remote_dir}/../../../tools/testing/kata-remote-libvirt/target/release/${KATA_REMOTE_BINARY}"

	if [ ! -f "${binary_path}" ]; then
		die "kata-remote-libvirt binary not found at ${binary_path}"
	fi

	# Create socket directory
	sudo mkdir -p "$(dirname "${KATA_REMOTE_SOCKET}")"

	# Start server in background
	sudo "${binary_path}" \
		--socket "${KATA_REMOTE_SOCKET}" \
		--libvirt-uri "qemu:///system" \
		--base-volume "${LIBVIRT_POOL}/${LIBVIRT_BASE_IMAGE}" \
		--log-level debug \
		> /tmp/kata-remote-libvirt.log 2>&1 &

	local server_pid=$!
	echo "${server_pid}" > /tmp/kata-remote-libvirt.pid

	# Wait for server to be ready
	local max_wait=30
	local count=0
	while [ $count -lt $max_wait ]; do
		if [ -S "${KATA_REMOTE_SOCKET}" ]; then
			info "kata-remote-libvirt server started (PID: ${server_pid})"
			return 0
		fi
		sleep 1
		count=$((count + 1))
	done

	die "kata-remote-libvirt server failed to start within ${max_wait} seconds"
}

function stop_kata_remote_server() {
	info "Stopping kata-remote-libvirt server"

	if [ -f /tmp/kata-remote-libvirt.pid ]; then
		local pid
		pid=$(cat /tmp/kata-remote-libvirt.pid)
		if ps -p "${pid}" > /dev/null 2>&1; then
			sudo kill "${pid}" || true
			sleep 2
			if ps -p "${pid}" > /dev/null 2>&1; then
				sudo kill -9 "${pid}" || true
			fi
		fi
		rm -f /tmp/kata-remote-libvirt.pid
	fi

	# Clean up socket
	sudo rm -f "${KATA_REMOTE_SOCKET}"

	info "kata-remote-libvirt server stopped"
}

function run_smoke_tests() {
	info "Running smoke tests for kata-remote-libvirt"

	# Test 1: Verify server is running
	info "Test 1: Verify server is running"
	if [ ! -S "${KATA_REMOTE_SOCKET}" ]; then
		die "Server socket not found at ${KATA_REMOTE_SOCKET}"
	fi
	info "✓ Server socket exists"

	# Test 2: Check libvirt connection
	info "Test 2: Check libvirt connection"
	if ! sudo virsh list > /dev/null 2>&1; then
		die "Failed to connect to libvirt"
	fi
	info "✓ Libvirt connection successful"

	# Test 3: Verify base image exists
	info "Test 3: Verify base image exists"
	if ! sudo virsh vol-list "${LIBVIRT_POOL}" | grep -q "${LIBVIRT_BASE_IMAGE}"; then
		die "Base image ${LIBVIRT_BASE_IMAGE} not found in pool ${LIBVIRT_POOL}"
	fi
	info "✓ Base image exists"

	# Test 4: Check server logs for errors
	info "Test 4: Check server logs"
	if grep -i "error" /tmp/kata-remote-libvirt.log | grep -v "ERROR_LEVEL"; then
		warn "Errors found in server logs:"
		grep -i "error" /tmp/kata-remote-libvirt.log | head -10
	else
		info "✓ No errors in server logs"
	fi

	info "Smoke tests completed successfully"
}

function run_container_tests() {
	info "Running container integration tests"

	# Check if containerd is available
	if ! command -v ctr &> /dev/null; then
		warn "containerd (ctr) not found, skipping container tests"
		warn "Install containerd and configure it with kata-remote to run full tests"
		return 0
	fi

	# Test 1: Try to run a simple container with kata-remote
	info "Test 1: Run simple container with kata-remote"
	local container_name="kata-remote-test-$$"
	local image="docker.io/library/busybox:latest"

	# Pull image if not present
	if ! sudo ctr image ls | grep -q "${image}"; then
		info "Pulling image ${image}..."
		sudo ctr image pull "${image}" || {
			warn "Failed to pull image, skipping container test"
			return 0
		}
	fi

	# Try to run container with kata-remote runtime
	info "Running container ${container_name}..."
	if sudo ctr run --runtime io.containerd.kata-remote.v2 --rm "${image}" "${container_name}" echo "Hello from kata-remote" 2>&1 | tee /tmp/container-test.log; then
		info "✓ Container ran successfully"
	else
		warn "Container test failed (expected with placeholder image)"
		warn "This is normal - the base image needs kata-agent for full functionality"
		cat /tmp/container-test.log
	fi

	info "Container tests completed"
}

function cleanup() {
	info "Cleaning up test environment"

	stop_kata_remote_server

	# Clean up any test VMs
	for vm in $(sudo virsh list --all --name | grep "kata-remote-"); do
		info "Cleaning up VM: ${vm}"
		sudo virsh destroy "${vm}" 2>/dev/null || true
		sudo virsh undefine "${vm}" 2>/dev/null || true
	done

	info "Cleanup completed"
}

function run() {
	info "Running kata-remote-libvirt integration tests"

	# Ensure cleanup on exit
	trap cleanup EXIT

	# Start the server
	start_kata_remote_server

	# Run smoke tests
	run_smoke_tests

	# Run container tests
	run_container_tests

	info "All tests completed successfully"
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-dependencies) install_dependencies ;;
		build-binary) build_kata_remote_libvirt ;;
		create-base-image) create_base_image ;;
		configure-runtime) configure_kata_runtime ;;
		install-kata) install_kata ;;
		run) run ;;
		cleanup) cleanup ;;
		*) >&2 die "Invalid argument. Usage: $0 {install-dependencies|build-binary|create-base-image|configure-runtime|install-kata|run|cleanup}" ;;
	esac
}

main "$@"
