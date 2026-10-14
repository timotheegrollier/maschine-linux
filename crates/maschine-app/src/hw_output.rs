//! LED + Display writer pour MK3.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::Receiver;
use maschine_proto::color::Color;
use maschine_proto::display::DisplayPixel;
use maschine_proto::hid_output::LedState;

/// Lance un thread qui envoie les LEDs via hidraw OUT (0x80/0x81).
/// Reçoit `LedState` via channel, envoie seulement quand change, au max 60Hz.
#[allow(dead_code)]
pub struct LedWriterHandle {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl LedWriterHandle {
    #[allow(dead_code)]
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Démarre le writer LEDs. `hidraw_path` est typiquement `/dev/hidraw3`.
pub fn spawn_led_writer(
    hidraw_path: PathBuf,
    rx: Receiver<LedState>,
) -> anyhow::Result<LedWriterHandle> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let join = std::thread::Builder::new()
        .name("maschine-led".to_string())
        .spawn(move || {
            if let Err(e) = led_loop(hidraw_path, rx, stop2) {
                log::error!("led writer: {e}");
            }
        })
        .map_err(|e| anyhow::anyhow!("spawn led writer: {e}"))?;
    Ok(LedWriterHandle {
        stop,
        join: Some(join),
    })
}

fn led_loop(
    hidraw_path: PathBuf,
    rx: Receiver<LedState>,
    stop: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    // Ouvre en RDWR — hidraw peut être ouvert plusieurs fois (read + write)
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&hidraw_path)
        .map_err(|e| anyhow::anyhow!("open hidraw write {}: {e}", hidraw_path.display()))?;
    use std::os::fd::AsRawFd;
    let fd = file.as_raw_fd();
    // passe en non-bloquant ? Pour write on peut rester bloquant mais avec timeout
    // On garde bloquant pour simplicité

    log::info!("LED writer ouvert sur {}", hidraw_path.display());

    // État initial : tout éteint puis arc-en-ciel
    let mut last_state = LedState::new();
    let _ = send_leds(fd, &last_state);

    // Petite animation de démarrage : chenillard
    {
        let mut demo = LedState::new();
        let palette = [
            6u8, 22, 30, 42, 50, 58, 10, 14, 26, 31, 34, 38, 11, 27, 33, 78,
        ];
        for (i, &col) in palette.iter().enumerate().take(16) {
            demo.set_pad(i, Color::from_index(col).unwrap_or(Color::OFF));
        }
        let _ = send_leds(fd, &demo);
        std::thread::sleep(Duration::from_millis(400));
        // Groups
        for g in 0..8 {
            demo.set_button_raw(
                maschine_proto::hid_output::offsets::GROUP_A + g,
                palette[g % palette.len()],
            );
        }
        let _ = send_leds(fd, &demo);
        std::thread::sleep(Duration::from_millis(400));
    }

    let mut pending: Option<LedState> = None;
    loop {
        if stop.load(Ordering::Relaxed) {
            // éteint proprement
            let _ = send_leds(fd, &LedState::new());
            break;
        }
        // Recv avec timeout 16ms (~60Hz)
        match rx.recv_timeout(Duration::from_millis(16)) {
            Ok(state) => {
                pending = Some(state);
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // si on a un pending, envoie
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                log::info!("LED channel disconnected");
                let _ = send_leds(fd, &LedState::new());
                break;
            }
        }
        if let Some(state) = pending.take() {
            if state != last_state {
                if let Err(e) = send_leds(fd, &state) {
                    log::warn!("LED send err: {e}");
                } else {
                    last_state = state;
                }
            }
        }
    }
    Ok(())
}

pub fn send_leds(fd: std::os::fd::RawFd, state: &LedState) -> anyhow::Result<()> {
    use maschine_proto::hid_output::{encode_report_80, encode_report_81_mapped};
    let r80 = encode_report_80(state)?;
    let r81 = encode_report_81_mapped(state, maschine_hardware::pad_output_hw_order())?;
    // hidraw write : inclut report ID
    // On tente avec ID, si EINVAL on retry sans ID
    unsafe {
        let ret80 = libc::write(fd, r80.as_ptr() as *const libc::c_void, r80.len());
        if ret80 < 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::EINVAL) {
                // retry sans ID
                let ret = libc::write(fd, r80[1..].as_ptr() as *const libc::c_void, r80.len() - 1);
                if ret < 0 {
                    return Err(anyhow::anyhow!(
                        "write 0x80 sans ID: {}",
                        std::io::Error::last_os_error()
                    ));
                }
            } else {
                return Err(anyhow::anyhow!("write 0x80: {e}"));
            }
        }
        let ret81 = libc::write(fd, r81.as_ptr() as *const libc::c_void, r81.len());
        if ret81 < 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::EINVAL) {
                let ret = libc::write(fd, r81[1..].as_ptr() as *const libc::c_void, r81.len() - 1);
                if ret < 0 {
                    return Err(anyhow::anyhow!(
                        "write 0x81 sans ID: {}",
                        std::io::Error::last_os_error()
                    ));
                }
            } else {
                return Err(anyhow::anyhow!("write 0x81: {e}"));
            }
        }
    }
    Ok(())
}

/// Test rapide : allume tous les pads en couleur, attends, éteint.
pub fn led_test_cycle(hidraw_path: &PathBuf, duration_ms: u64) -> anyhow::Result<()> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(hidraw_path)?;
    use std::os::fd::AsRawFd;
    let fd = file.as_raw_fd();
    let mut state = LedState::new();
    let colors = [6u8, 22, 30, 42, 50, 58, 78, 10];
    for &c in &colors {
        state.set_all_pads(Color::from_index(c).unwrap());
        send_leds(fd, &state)?;
        std::thread::sleep(Duration::from_millis(duration_ms / colors.len() as u64));
    }
    // chenillard pad par pad
    for i in 0..16 {
        let mut s = LedState::new();
        s.set_pad(i, Color::from_index(6).unwrap());
        send_leds(fd, &s)?;
        std::thread::sleep(Duration::from_millis(80));
    }
    send_leds(fd, &LedState::new())?;
    Ok(())
}

/// Display writer via rusb bulk (IF5 EP 0x04).
#[cfg(feature = "usb")]
pub mod display {
    use super::*;
    use maschine_hardware::usb::BulkHandle;
    use maschine_proto::display::{DisplayFrame, DisplayId};

    pub fn show_solid(color: DisplayPixel, duration_ms: u64) -> anyhow::Result<()> {
        let mut bulk = BulkHandle::open().map_err(|e| anyhow::anyhow!("bulk open: {e}"))?;
        let left = DisplayFrame::solid(DisplayId::Left, color);
        let right = DisplayFrame::solid(DisplayId::Right, color);
        log::info!("Envoi écrans solid {:?}...", color.0);
        bulk.send_frame(&left)
            .map_err(|e| anyhow::anyhow!("bulk left: {e}"))?;
        bulk.send_frame(&right)
            .map_err(|e| anyhow::anyhow!("bulk right: {e}"))?;
        std::thread::sleep(Duration::from_millis(duration_ms));
        // efface
        let black = DisplayFrame::solid(DisplayId::Left, DisplayPixel::BLACK);
        let black2 = DisplayFrame::solid(DisplayId::Right, DisplayPixel::BLACK);
        let _ = bulk.send_frame(&black);
        let _ = bulk.send_frame(&black2);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn show_text(left_text: &str, right_text: &str) -> anyhow::Result<()> {
        let mut bulk = BulkHandle::open().map_err(|e| anyhow::anyhow!("bulk open: {e}"))?;
        let bg = DisplayPixel::from_rgb(18, 18, 24);
        let fg = DisplayPixel::WHITE;

        let mut left = DisplayFrame::solid(maschine_proto::display::DisplayId::Left, bg);
        maschine_engine::display_render::draw_text(&mut left, 20, 120, left_text, fg, bg);

        let mut right = DisplayFrame::solid(maschine_proto::display::DisplayId::Right, bg);
        maschine_engine::display_render::draw_text(&mut right, 20, 120, right_text, fg, bg);

        log::info!("Écrans texte: left='{left_text}' right='{right_text}'");
        bulk.send_frame(&left)?;
        bulk.send_frame(&right)?;
        Ok(())
    }
}

#[cfg(not(feature = "usb"))]
pub mod display {
    use super::*;
    pub fn show_solid(_color: DisplayPixel, _duration_ms: u64) -> anyhow::Result<()> {
        anyhow::bail!("feature `usb` non activée — rebuild avec --features usb")
    }
    pub fn show_text(_left_text: &str, _right_text: &str) -> anyhow::Result<()> {
        anyhow::bail!("feature `usb` non activée")
    }
}
