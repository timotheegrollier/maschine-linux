//! Bulk display via `rusb` (libusb) — optionnel, feature `usb`.
//!
//! Mock si pas de hardware. Gère IF5 Vendor Bulk EP 0x04.

#[cfg(feature = "usb")]
use rusb::{Context, DeviceHandle, UsbContext};

use crate::{HardwareError, Result};
use maschine_proto::display::{encode_bulk_transfer, DisplayFrame};

/// Handle bulk display.
#[cfg(feature = "usb")]
pub struct BulkHandle {
    handle: DeviceHandle<Context>,
    iface: u8,
}

#[cfg(feature = "usb")]
impl BulkHandle {
    /// Ouvre le bulk display (IF5) pour la MK3.
    pub fn open() -> Result<Self> {
        let ctx = Context::new().map_err(|e| HardwareError::Usb(format!("rusb context: {e}")))?;
        let devices = ctx
            .devices()
            .map_err(|e| HardwareError::Usb(format!("devices: {e}")))?;
        for device in devices.iter() {
            let desc = device
                .device_descriptor()
                .map_err(|e| HardwareError::Usb(format!("{e}")))?;
            if desc.vendor_id() == maschine_proto::consts::VENDOR_ID
                && desc.product_id() == maschine_proto::consts::PRODUCT_ID_MK3
            {
                let handle = device
                    .open()
                    .map_err(|e| HardwareError::Usb(format!("open: {e}")))?;
                let iface = 5u8;
                let _ = handle.set_auto_detach_kernel_driver(true);
                handle
                    .claim_interface(iface)
                    .map_err(|e| HardwareError::Usb(format!("claim IF5: {e}")))?;
                return Ok(Self { handle, iface });
            }
        }
        Err(HardwareError::NotFound(
            "MK3 bulk IF5 non trouvé".to_string(),
        ))
    }

    /// Envoie une frame.
    pub fn send_frame(&mut self, frame: &DisplayFrame) -> Result<()> {
        let data = encode_bulk_transfer(frame).map_err(|e| HardwareError::Other(format!("{e}")))?;
        // EP 0x04 OUT — un seul transfert bulk complet (261148 octets)
        // libusb gère le découpage en paquets USB 512B en interne.
        // Référence : ni-controllers-lib transferOut(endpoint, buffer) envoie tout d'un coup.
        let n = self
            .handle
            .write_bulk(0x04, &data, std::time::Duration::from_millis(1000))
            .map_err(|e| HardwareError::Usb(format!("bulk write: {e}")))?;
        if n != data.len() {
            return Err(HardwareError::Usb(format!(
                "bulk short write {}/{}",
                n,
                data.len()
            )));
        }
        Ok(())
    }
}

#[cfg(feature = "usb")]
impl Drop for BulkHandle {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(self.iface);
        let _ = self.handle.attach_kernel_driver(self.iface);
    }
}

/// Stub sans feature `usb`.
#[cfg(not(feature = "usb"))]
pub struct BulkHandle;

#[cfg(not(feature = "usb"))]
impl BulkHandle {
    /// Ouvre — erreur si feature non activée.
    pub fn open() -> Result<Self> {
        Err(HardwareError::Other(
            "feature `usb` non activée — compile avec --features usb + installe libusb1-devel"
                .to_string(),
        ))
    }

    /// Envoie — stub.
    pub fn send_frame(&mut self, _frame: &DisplayFrame) -> Result<()> {
        Err(HardwareError::Other("bulk not available".to_string()))
    }
}
