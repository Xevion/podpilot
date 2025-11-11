#!/bin/bash

df -h

echo "Freeing up disk space..."

# Helper function: delete directory using find (faster for large trees), fallback to rm
cleanup_fast() {
    for dir in "$@"; do
        if [ -d "$dir" ]; then
            # Try find -delete first (faster for large directories)
            sudo find "$dir" -delete 2>/dev/null || sudo rm -rf "$dir" 2>/dev/null || true
        fi
    done
}

# Helper function: delete in background
cleanup_bg() {
    cleanup_fast "$@" &
}

# Batch 1: Largest single directory - hostedtoolcache (contains Python, PyPy, Node, Ruby, CodeQL)
echo "Batch 1: Removing hostedtoolcache..."
cleanup_bg /opt/hostedtoolcache
wait

# Batch 2: Language toolchains and SDKs
echo "Batch 2: Removing language toolchains..."
cleanup_bg /usr/lib/jvm
cleanup_bg /usr/share/dotnet
cleanup_bg /usr/share/swift
cleanup_bg /usr/local/.ghcup /opt/ghc
cleanup_bg /usr/local/julia*
cleanup_bg /usr/local/lib/android
cleanup_bg /usr/local/go
cleanup_bg /usr/share/miniconda
wait

# Batch 3: Rust installations (multiple user directories)
echo "Batch 3: Removing Rust toolchains..."
cleanup_bg /home/runner/.rustup /home/runner/.cargo
cleanup_bg /home/packer/.rustup /home/packer/.cargo
cleanup_bg /etc/skel/.rustup /etc/skel/.cargo
wait

# Batch 4: Cloud and development tools
echo "Batch 4: Removing cloud tools..."
cleanup_bg /opt/az
cleanup_bg /usr/share/az_*
cleanup_bg /usr/local/aws-sam-cli
cleanup_bg /home/linuxbrew/.linuxbrew
cleanup_bg /usr/local/share/powershell
wait

# Batch 5: Browsers
echo "Batch 5: Removing browsers..."
cleanup_bg /usr/local/share/chromium
cleanup_bg /opt/microsoft /opt/google
cleanup_bg /usr/lib/firefox
wait

# Batch 6: Databases
echo "Batch 6: Removing databases..."
cleanup_bg /var/lib/mysql /usr/sbin/mysqld
cleanup_bg /usr/lib/postgresql
wait

# Batch 7: System components and old compiler versions
echo "Batch 7: Removing system components..."
cleanup_bg /usr/lib/snapd /var/lib/snapd
cleanup_bg /usr/lib/llvm-16 /usr/lib/llvm-17
cleanup_bg /usr/lib/gcc/x86_64-linux-gnu/12 /usr/lib/gcc/x86_64-linux-gnu/13
cleanup_bg /usr/libexec/gcc/x86_64-linux-gnu/13
wait

# Batch 8: Docker cleanup (can be slow, run last)
echo "Batch 8: Cleaning Docker..."
docker system prune --all --force --volumes || true
docker builder prune --all --force || true

df -h
