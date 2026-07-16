//! Hardware accelerator detection for local inference
//!
//! Detects GPUs and NPUs available on the edge node so the agent can
//! choose the best local inference backend:
//! - GPUs via DRM sysfs (`/sys/class/drm`) and the NVIDIA proc interface
//! - NPUs via the Linux accel subsystem (`/sys/class/accel`) and
//!   vendor-specific device nodes (Rockchip, Hailo)
//!
//! Detection is read-only and never fails: on non-Linux platforms or
//! locked-down systems it simply returns an empty list.

use std::fmt;
use std::fs;
use std::path::Path;

/// Kind of hardware accelerator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceleratorKind {
    Gpu,
    Npu,
}

/// Accelerator vendor
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
    Qualcomm,
    Rockchip,
    Hailo,
    Habana,
    Other(String),
}

impl fmt::Display for Vendor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Vendor::Nvidia => write!(f, "NVIDIA"),
            Vendor::Amd => write!(f, "AMD"),
            Vendor::Intel => write!(f, "Intel"),
            Vendor::Qualcomm => write!(f, "Qualcomm"),
            Vendor::Rockchip => write!(f, "Rockchip"),
            Vendor::Hailo => write!(f, "Hailo"),
            Vendor::Habana => write!(f, "Habana"),
            Vendor::Other(s) => write!(f, "{}", s),
        }
    }
}

/// A detected hardware accelerator
#[derive(Debug, Clone)]
pub struct Accelerator {
    pub kind: AcceleratorKind,
    pub vendor: Vendor,
    /// Human-readable name (marketing name when available, PCI id otherwise)
    pub name: String,
    /// Device node or sysfs path this was detected from
    pub device_path: String,
    /// Kernel driver bound to the device, when known
    pub driver: Option<String>,
    /// sysfs device directory (used to locate hwmon sensors), when known
    pub sysfs_device: Option<String>,
}

/// Point-in-time sensor readings for an accelerator
#[derive(Debug, Clone, Default)]
pub struct SensorReading {
    pub temp_c: Option<f32>,
    pub power_w: Option<f32>,
    pub utilization_pct: Option<f32>,
}

impl fmt::Display for SensorReading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(t) = self.temp_c {
            parts.push(format!("{:.0}°C", t));
        }
        if let Some(p) = self.power_w {
            parts.push(format!("{:.1}W", p));
        }
        if let Some(u) = self.utilization_pct {
            parts.push(format!("{:.0}% util", u));
        }
        if parts.is_empty() {
            write!(f, "no sensors")
        } else {
            write!(f, "{}", parts.join(", "))
        }
    }
}

impl Accelerator {
    /// Whether llama.cpp can offload transformer layers to this device
    /// (requires a CUDA/HIP/Vulkan-enabled build).
    pub fn supports_llama_offload(&self) -> bool {
        self.kind == AcceleratorKind::Gpu
            && matches!(self.vendor, Vendor::Nvidia | Vendor::Amd | Vendor::Intel)
    }

    /// Read current temperature/power/utilization for this device.
    ///
    /// Uses hwmon sysfs when the driver exposes it (amdgpu, intel, most
    /// accel devices); falls back to `nvidia-smi` for the proprietary
    /// NVIDIA driver, which has no hwmon interface. Missing sensors
    /// yield `None` fields, never errors.
    pub fn read_sensors(&self) -> SensorReading {
        // hwmon first: cheap, no subprocess
        if let Some(ref sysfs) = self.sysfs_device {
            let reading = read_hwmon_sensors(Path::new(sysfs));
            if reading.temp_c.is_some() || reading.power_w.is_some() {
                return reading;
            }
        }
        if self.vendor == Vendor::Nvidia {
            return read_nvidia_smi_sensors();
        }
        SensorReading::default()
    }
}

/// Read temp/power from a device's hwmon directory (sysfs, millidegrees /
/// microwatts).
fn read_hwmon_sensors(sysfs_device: &Path) -> SensorReading {
    let mut reading = SensorReading::default();
    let Ok(entries) = fs::read_dir(sysfs_device.join("hwmon")) else {
        return reading;
    };

    for entry in entries.flatten() {
        let hwmon = entry.path();
        if reading.temp_c.is_none() {
            reading.temp_c = sysfs_read(&hwmon.join("temp1_input"))
                .and_then(|v| v.parse::<f32>().ok())
                .map(|milli_c| milli_c / 1000.0);
        }
        if reading.power_w.is_none() {
            // power1_average (or power1_input) is in microwatts
            reading.power_w = ["power1_average", "power1_input"]
                .iter()
                .find_map(|f| sysfs_read(&hwmon.join(f)))
                .and_then(|v| v.parse::<f32>().ok())
                .map(|micro_w| micro_w / 1_000_000.0);
        }
    }

    // GPU busy percentage lives next to hwmon on amdgpu
    reading.utilization_pct =
        sysfs_read(&sysfs_device.join("gpu_busy_percent")).and_then(|v| v.parse::<f32>().ok());

    reading
}

/// Query the proprietary NVIDIA driver via nvidia-smi (no hwmon support).
fn read_nvidia_smi_sensors() -> SensorReading {
    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=temperature.gpu,power.draw,utilization.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output();

    let Ok(output) = output else {
        return SensorReading::default();
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(line) = stdout.lines().next() else {
        return SensorReading::default();
    };

    let mut fields = line.split(',').map(|f| f.trim().parse::<f32>().ok());
    SensorReading {
        temp_c: fields.next().flatten(),
        power_w: fields.next().flatten(),
        utilization_pct: fields.next().flatten(),
    }
}

impl fmt::Display for Accelerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            AcceleratorKind::Gpu => "GPU",
            AcceleratorKind::Npu => "NPU",
        };
        write!(f, "[{}] {} {}", kind, self.vendor, self.name)?;
        if let Some(ref drv) = self.driver {
            write!(f, " (driver: {})", drv)?;
        }
        Ok(())
    }
}

/// Detect all GPUs and NPUs on this node.
pub fn detect_accelerators() -> Vec<Accelerator> {
    let mut found = Vec::new();
    found.extend(detect_drm_gpus());
    found.extend(detect_accel_npus());
    found.extend(detect_misc_npus());
    found
}

/// Map a PCI vendor id to a `Vendor`.
fn vendor_from_pci_id(vendor_id: &str) -> Vendor {
    match vendor_id {
        "0x10de" => Vendor::Nvidia,
        "0x1002" | "0x1022" => Vendor::Amd,
        "0x8086" => Vendor::Intel,
        "0x17cb" => Vendor::Qualcomm,
        other => Vendor::Other(other.to_string()),
    }
}

/// Read and trim a sysfs attribute.
fn sysfs_read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// Extract `KEY=value` from a uevent file.
fn uevent_value(uevent_path: &Path, key: &str) -> Option<String> {
    let content = fs::read_to_string(uevent_path).ok()?;
    content
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{}=", key)))
        .map(|v| v.to_string())
}

/// GPUs via the DRM subsystem: /sys/class/drm/card<N>/device
fn detect_drm_gpus() -> Vec<Accelerator> {
    let mut gpus = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return gpus;
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        // Only bare "cardN" entries — skip connectors like "card1-DP-1"
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }

        let device_dir = entry.path().join("device");
        let Some(vendor_id) = sysfs_read(&device_dir.join("vendor")) else {
            continue;
        };
        let vendor = vendor_from_pci_id(&vendor_id);
        let device_id = sysfs_read(&device_dir.join("device")).unwrap_or_default();
        let driver = uevent_value(&device_dir.join("uevent"), "DRIVER");

        let pretty_name = match vendor {
            Vendor::Nvidia => nvidia_model_name().unwrap_or_else(|| format!("GPU {}", device_id)),
            _ => format!("GPU {}", device_id),
        };

        gpus.push(Accelerator {
            kind: AcceleratorKind::Gpu,
            vendor,
            name: pretty_name,
            device_path: format!("/dev/dri/{}", name),
            driver,
            sysfs_device: Some(device_dir.to_string_lossy().to_string()),
        });
    }
    gpus
}

/// Marketing name from /proc/driver/nvidia/gpus/*/information ("Model:" line).
fn nvidia_model_name() -> Option<String> {
    let entries = fs::read_dir("/proc/driver/nvidia/gpus").ok()?;
    for entry in entries.flatten() {
        let info = fs::read_to_string(entry.path().join("information")).ok()?;
        for line in info.lines() {
            if let Some(model) = line.strip_prefix("Model:") {
                return Some(model.trim().to_string());
            }
        }
    }
    None
}

/// NPUs via the Linux compute-accelerator subsystem: /sys/class/accel/accel<N>
fn detect_accel_npus() -> Vec<Accelerator> {
    let mut npus = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/accel") else {
        return npus;
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if !name.starts_with("accel") || name.contains('-') {
            continue;
        }

        let device_dir = entry.path().join("device");
        let driver = uevent_value(&device_dir.join("uevent"), "DRIVER");

        let (vendor, pretty_name) = match driver.as_deref() {
            Some("amdxdna") => (Vendor::Amd, "XDNA NPU (Ryzen AI)".to_string()),
            Some("intel_vpu") => (Vendor::Intel, "NPU (VPU)".to_string()),
            Some("qaic") => (Vendor::Qualcomm, "Cloud AI accelerator".to_string()),
            Some("habanalabs") => (Vendor::Habana, "Gaudi accelerator".to_string()),
            Some(other) => (
                Vendor::Other(other.to_string()),
                format!("accelerator ({})", other),
            ),
            None => {
                // Fall back to PCI vendor id when no driver is bound
                let vendor = sysfs_read(&device_dir.join("vendor"))
                    .map(|v| vendor_from_pci_id(&v))
                    .unwrap_or(Vendor::Other("unknown".to_string()));
                (vendor, "accelerator (unbound)".to_string())
            }
        };

        npus.push(Accelerator {
            kind: AcceleratorKind::Npu,
            vendor,
            name: pretty_name,
            device_path: format!("/dev/accel/{}", name),
            driver,
            sysfs_device: Some(device_dir.to_string_lossy().to_string()),
        });
    }
    npus
}

/// NPUs that predate the accel subsystem and expose bespoke device nodes.
fn detect_misc_npus() -> Vec<Accelerator> {
    let mut npus = Vec::new();

    for (glob_dir, prefix, vendor, name) in [
        ("/dev", "rknpu", Vendor::Rockchip, "RKNPU"),
        ("/dev", "hailo", Vendor::Hailo, "Hailo NPU"),
    ] {
        let Ok(entries) = fs::read_dir(glob_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let node = file_name.to_string_lossy();
            if node.starts_with(prefix) {
                npus.push(Accelerator {
                    kind: AcceleratorKind::Npu,
                    vendor: vendor.clone(),
                    name: name.to_string(),
                    device_path: format!("{}/{}", glob_dir, node),
                    driver: None,
                    sysfs_device: None,
                });
            }
        }
    }

    npus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_does_not_panic() {
        // Detection must be safe on any host, with or without accelerators
        let accels = detect_accelerators();
        for a in &accels {
            assert!(!a.name.is_empty());
            assert!(!a.device_path.is_empty());
        }
    }

    #[test]
    fn test_sensor_reading_never_panics() {
        for accel in detect_accelerators() {
            // Readings are best-effort; just verify they format cleanly
            let _ = accel.read_sensors().to_string();
        }
    }

    #[test]
    fn test_sensor_display() {
        let empty = SensorReading::default();
        assert_eq!(empty.to_string(), "no sensors");

        let full = SensorReading {
            temp_c: Some(45.0),
            power_w: Some(3.25),
            utilization_pct: Some(12.0),
        };
        assert_eq!(full.to_string(), "45°C, 3.2W, 12% util");
    }

    #[test]
    fn test_vendor_mapping() {
        assert_eq!(vendor_from_pci_id("0x10de"), Vendor::Nvidia);
        assert_eq!(vendor_from_pci_id("0x1002"), Vendor::Amd);
        assert_eq!(vendor_from_pci_id("0x8086"), Vendor::Intel);
        assert!(matches!(vendor_from_pci_id("0xdead"), Vendor::Other(_)));
    }

    #[test]
    fn test_llama_offload_capability() {
        let gpu = Accelerator {
            kind: AcceleratorKind::Gpu,
            vendor: Vendor::Nvidia,
            name: "test".to_string(),
            device_path: "/dev/dri/card0".to_string(),
            driver: None,
            sysfs_device: None,
        };
        assert!(gpu.supports_llama_offload());

        let npu = Accelerator {
            kind: AcceleratorKind::Npu,
            vendor: Vendor::Amd,
            name: "test".to_string(),
            device_path: "/dev/accel/accel0".to_string(),
            driver: None,
            sysfs_device: None,
        };
        assert!(!npu.supports_llama_offload());
    }
}
