use podpilot_common::types::GpuInfo;
use std::process::Command;
use tracing::{debug, warn};

/// Detect GPU information using nvidia-smi
pub fn detect_gpu() -> GpuInfo {
    match detect_nvidia_gpu() {
        Ok(gpu_info) => {
            debug!("Detected GPU: {}", gpu_info.name);
            gpu_info
        }
        Err(e) => {
            warn!("Failed to detect GPU, using placeholder: {}", e);
            GpuInfo {
                name: "Unknown GPU".to_string(),
                memory_gb: 0.0,
                cuda_version: "unknown".to_string(),
                compute_capability: None,
            }
        }
    }
}

/// Try to detect NVIDIA GPU using nvidia-smi
fn detect_nvidia_gpu() -> anyhow::Result<GpuInfo> {
    // Query GPU name
    let name_output = Command::new("nvidia-smi")
        .args(&["--query-gpu=name", "--format=csv,noheader"])
        .output()?;

    if !name_output.status.success() {
        anyhow::bail!("nvidia-smi failed to query GPU name");
    }

    let name = String::from_utf8(name_output.stdout)?
        .trim()
        .lines()
        .next()
        .unwrap_or("Unknown NVIDIA GPU")
        .to_string();

    // Query memory in MB, convert to GB
    let memory_output = Command::new("nvidia-smi")
        .args(&[
            "--query-gpu=memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()?;

    let memory_mb: f32 = String::from_utf8(memory_output.stdout)?
        .trim()
        .lines()
        .next()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0.0);

    let memory_gb = (memory_mb / 1024.0 * 100.0).round() / 100.0; // Round to 2 decimals

    // Query CUDA version from nvidia-smi
    // nvidia-smi reports the maximum supported CUDA version in its header output
    let cuda_output = Command::new("nvidia-smi").output()?;

    // Parse CUDA version from nvidia-smi output header (e.g., "CUDA Version: 13.0")
    let cuda_version = String::from_utf8(cuda_output.stdout)?
        .lines()
        .find(|line| line.contains("CUDA Version"))
        .and_then(|line| {
            line.split("CUDA Version:")
                .nth(1)
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Query compute capability
    let capability_output = Command::new("nvidia-smi")
        .args(&[
            "--query-gpu=compute_cap",
            "--format=csv,noheader",
        ])
        .output();

    let compute_capability = if let Ok(output) = capability_output {
        String::from_utf8(output.stdout)
            .ok()
            .and_then(|s| s.trim().lines().next().map(|l| l.to_string()))
    } else {
        None
    };

    Ok(GpuInfo {
        name,
        memory_gb,
        cuda_version,
        compute_capability,
    })
}
