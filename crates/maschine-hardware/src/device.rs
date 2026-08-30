//! Découverte des devices Maschine.

use std::path::PathBuf;

use crate::{HardwareError, Result};

/// Info device détecté.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// VID
    pub vendor_id: u16,
    /// PID
    pub product_id: u16,
    /// Serial (ex: 1B64AB9B)
    pub serial: Option<String>,
    /// hidraw path si dispo (ex: /dev/hidraw3)
    pub hidraw_path: Option<PathBuf>,
    /// bus/port (ex: 1-1)
    pub bus_port: Option<String>,
    /// Manufacturer string
    pub manufacturer: Option<String>,
    /// Product string
    pub product: Option<String>,
}

impl DeviceInfo {
    /// Vrai si c'est la MK3.
    #[must_use]
    pub fn is_mk3(&self) -> bool {
        self.vendor_id == maschine_proto::consts::VENDOR_ID
            && self.product_id == maschine_proto::consts::PRODUCT_ID_MK3
    }

    /// Vrai si c'est la Mikro MK3.
    #[must_use]
    pub fn is_mikro_mk3(&self) -> bool {
        self.vendor_id == maschine_proto::consts::VENDOR_ID
            && self.product_id == maschine_proto::consts::PRODUCT_ID_MIKRO_MK3
    }
}

/// Handle ouvert — pour l'instant on garde seulement DeviceInfo + hidraw fd.
/// Le bulk (IF5) sera ouvert via `crate::usb` si feature `usb`.
#[derive(Debug)]
pub struct MaschineDevice {
    /// Info
    pub info: DeviceInfo,
}

impl MaschineDevice {
    /// Ouvre depuis un DeviceInfo (vérifie accès hidraw).
    pub fn open(info: DeviceInfo) -> Result<Self> {
        if let Some(ref p) = info.hidraw_path {
            // test ouverture non-bloquante pour détecter permissions
            match std::fs::OpenOptions::new().read(true).open(p) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Err(HardwareError::Permission(format!(
                        "{}: {} — sudo cp config/udev/99-maschine-mk3.rules /etc/udev/rules.d/ && sudo udevadm trigger; groups doit contenir plugdev/input",
                        p.display(),
                        e
                    )));
                }
                Err(_) => {} // on laisse passer, sera retenté par hidraw thread
            }
        }
        Ok(Self { info })
    }
}

/// Scanne `/sys/class/hidraw` + `lsusb` fallback pour trouver les MK3.
pub fn find_maschine_devices() -> Result<Vec<DeviceInfo>> {
    let mut out = Vec::new();
    let hidraw_dir = std::path::Path::new("/sys/class/hidraw");
    if let Ok(entries) = std::fs::read_dir(hidraw_dir) {
        for entry in entries.flatten() {
            let uevent_path = entry.path().join("device/uevent");
            let Ok(content) = std::fs::read_to_string(&uevent_path) else {
                continue;
            };
            let mut vid = None;
            let mut pid = None;
            let mut serial = None;
            let mut hid_name = None;
            for line in content.lines() {
                if let Some(v) = line.strip_prefix("HID_ID=") {
                    // format 0003:000017CC:00001600
                    let parts: Vec<_> = v.split(':').collect();
                    if parts.len() == 3 {
                        if let Ok(vi) = u16::from_str_radix(parts[1].trim_start_matches('0'), 16) {
                            vid = Some(vi);
                        }
                        if let Ok(pi) = u16::from_str_radix(parts[2].trim_start_matches('0'), 16) {
                            pid = Some(pi);
                        }
                    }
                } else if let Some(v) = line.strip_prefix("HID_NAME=") {
                    hid_name = Some(v.to_string());
                } else if let Some(v) = line.strip_prefix("HID_UNIQ=") {
                    if !v.is_empty() {
                        serial = Some(v.to_string());
                    }
                }
            }
            if let (Some(v), Some(p)) = (vid, pid) {
                if v == maschine_proto::consts::VENDOR_ID
                    && (p == maschine_proto::consts::PRODUCT_ID_MK3
                        || p == maschine_proto::consts::PRODUCT_ID_MIKRO_MK3)
                {
                    let hidraw_path =
                        PathBuf::from(format!("/dev/{}", entry.file_name().to_string_lossy()));
                    // bus_port via symlink phys
                    let phys = std::fs::read_to_string(entry.path().join("device/phys"))
                        .unwrap_or_default();
                    // extraire bus/port depuis phys ou depuis modalias
                    let bus_port = if phys.contains("usb-") {
                        Some(phys.trim().to_string())
                    } else {
                        None
                    };
                    out.push(DeviceInfo {
                        vendor_id: v,
                        product_id: p,
                        serial,
                        hidraw_path: Some(hidraw_path),
                        bus_port,
                        manufacturer: Some("Native Instruments".to_string()),
                        product: hid_name,
                    });
                }
            }
        }
    }

    // Fallback : si rien trouvé via hidraw, scanner /sys/bus/usb/devices pour au moins lister le device audio
    if out.is_empty() {
        if let Ok(entries) = std::fs::read_dir("/sys/bus/usb/devices") {
            for entry in entries.flatten() {
                let uevent = entry.path().join("uevent");
                let Ok(content) = std::fs::read_to_string(&uevent) else {
                    continue;
                };
                let mut vid = None;
                let mut pid = None;
                for line in content.lines() {
                    if let Some(v) = line.strip_prefix("PRODUCT=") {
                        let parts: Vec<_> = v.split('/').collect();
                        if parts.len() >= 2 {
                            if let Ok(vi) = u16::from_str_radix(parts[0], 16) {
                                vid = Some(vi);
                            }
                            if let Ok(pi) = u16::from_str_radix(parts[1], 16) {
                                pid = Some(pi);
                            }
                        }
                    }
                }
                if let (Some(v), Some(p)) = (vid, pid) {
                    if v == maschine_proto::consts::VENDOR_ID
                        && (p == maschine_proto::consts::PRODUCT_ID_MK3
                            || p == maschine_proto::consts::PRODUCT_ID_MIKRO_MK3)
                    {
                        let serial = std::fs::read_to_string(entry.path().join("serial"))
                            .ok()
                            .map(|s| s.trim().to_string());
                        out.push(DeviceInfo {
                            vendor_id: v,
                            product_id: p,
                            serial,
                            hidraw_path: None,
                            bus_port: Some(entry.file_name().to_string_lossy().to_string()),
                            manufacturer: Some("Native Instruments".to_string()),
                            product: Some("Maschine MK3".to_string()),
                        });
                    }
                }
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_does_not_crash_without_hardware() {
        let devs = find_maschine_devices().unwrap();
        // peut être 0 ou 1 selon si MK3 branchée — on vérifie juste que ça ne panic pas
        for d in devs {
            assert_eq!(d.vendor_id, 0x17cc);
        }
    }
}
