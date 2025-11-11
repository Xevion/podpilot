#!/bin/bash

df -h

echo "Freeing up disk space..."

# Helper function: delete directory using rm -rf (fastest for whole directory trees)
cleanup_fast() {
    for dir in "$@"; do
        if [ -d "$dir" ]; then
            # Optional: show size before deletion (enable with SHOW_SIZES=1)
            if [ "${SHOW_SIZES:-0}" = "1" ]; then
                size=$(du -sh "$dir" 2>/dev/null | cut -f1)
                echo "  Removing $dir ($size)..."
            fi
            sudo rm -rf "$dir" 2>/dev/null || true
        fi
    done
}

# Helper function: delete in background
cleanup_bg() {
    cleanup_fast "$@" &
}

# Batch 1: Largest single directory - hostedtoolcache (contains Python, PyPy, Node, Ruby, CodeQL)
echo "Batch 1: Removing hostedtoolcache..."
batch_start=$SECONDS
cleanup_bg /opt/hostedtoolcache
wait
echo "  -> $((SECONDS - batch_start))s"

# Batch 2: Language toolchains and SDKs (split into sub-groups to reduce I/O contention)
echo "Batch 2: Removing language toolchains..."
batch_start=$SECONDS
# Sub-group 1: Largest directories
cleanup_bg /usr/lib/jvm
cleanup_bg /usr/share/dotnet
wait
# Sub-group 2: Medium-large directories
cleanup_bg /usr/local/lib/android
cleanup_bg /usr/share/swift
wait
# Sub-group 3: Medium directories
cleanup_bg /usr/local/.ghcup /opt/ghc
cleanup_bg /usr/local/julia*
wait
# Sub-group 4: Smaller directories
cleanup_bg /usr/local/go
cleanup_bg /usr/share/miniconda
wait
echo "  -> $((SECONDS - batch_start))s"

# Batch 3: Rust installations (multiple user directories)
echo "Batch 3: Removing Rust toolchains..."
batch_start=$SECONDS
cleanup_bg /home/runner/.rustup /home/runner/.cargo
cleanup_bg /home/packer/.rustup /home/packer/.cargo
cleanup_bg /etc/skel/.rustup /etc/skel/.cargo
wait
echo "  -> $((SECONDS - batch_start))s"

# Batch 4: Cloud and development tools (limit parallelization)
echo "Batch 4: Removing cloud tools..."
batch_start=$SECONDS
# Sub-group 1: Azure tools
cleanup_bg /opt/az
cleanup_bg /usr/share/az_*
wait
# Sub-group 2: Other cloud/dev tools
cleanup_bg /usr/local/aws-sam-cli
cleanup_bg /home/linuxbrew/.linuxbrew
wait
# Sub-group 3: PowerShell
cleanup_bg /usr/local/share/powershell
wait
echo "  -> $((SECONDS - batch_start))s"

# Batch 5: Browsers
echo "Batch 5: Removing browsers..."
batch_start=$SECONDS
cleanup_bg /usr/local/share/chromium
cleanup_bg /opt/microsoft /opt/google
cleanup_bg /usr/lib/firefox
wait
echo "  -> $((SECONDS - batch_start))s"

# Batch 6: Databases
echo "Batch 6: Removing databases..."
batch_start=$SECONDS
cleanup_bg /var/lib/mysql /usr/sbin/mysqld
cleanup_bg /usr/lib/postgresql
wait
echo "  -> $((SECONDS - batch_start))s"

# Batch 7: System components and old compiler versions
echo "Batch 7: Removing system components..."
batch_start=$SECONDS
cleanup_bg /usr/lib/snapd /var/lib/snapd
cleanup_bg /usr/lib/llvm-16 /usr/lib/llvm-17
cleanup_bg /usr/lib/gcc/x86_64-linux-gnu/12 /usr/lib/gcc/x86_64-linux-gnu/13
cleanup_bg /usr/libexec/gcc/x86_64-linux-gnu/13
wait
echo "  -> $((SECONDS - batch_start))s"

# Batch 8: Docker cleanup (direct removal for predictable timing)
echo "Batch 8: Cleaning Docker..."
batch_start=$SECONDS
# Direct removal is faster and more predictable than docker system prune
sudo rm -rf /var/lib/docker 2>/dev/null || true
sudo systemctl restart docker 2>/dev/null || true
echo "  -> $((SECONDS - batch_start))s"

df -h
