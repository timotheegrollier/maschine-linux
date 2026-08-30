//! Poll hidraw `/dev/hidraw*` — thread temps-réel, 1ms.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use maschine_proto::hid_input::{decode_report_01, decode_report_02};

use crate::device::DeviceInfo;
use crate::events::{pad_to_state, HardwareEvent};
use crate::{HardwareError, Result};

/// Config hidraw.
#[derive(Debug, Clone)]
pub struct HidrawConfig {
    /// Poll interval (défaut 1ms → bInterval HS)
    pub poll_interval: Duration,
    /// Debounce boutons (défaut 4ms)
    pub debounce: Duration,
    /// Taille ring buffer channel
    pub channel_cap: usize,
}

impl Default for HidrawConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(1),
            debounce: Duration::from_millis(4),
            channel_cap: 1024,
        }
    }
}

/// Handle du thread hidraw — `stop()` pour arrêter.
pub struct HidrawHandle {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl HidrawHandle {
    /// Arrête le thread.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for HidrawHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // on ne join pas en Drop pour éviter blocage — caller doit appeler stop()
    }
}

/// Démarre le poll hidraw en arrière-plan. Envoie `HardwareEvent` sur `tx`.
pub fn spawn_hidraw_thread(
    info: DeviceInfo,
    cfg: HidrawConfig,
    tx: Sender<HardwareEvent>,
) -> Result<HidrawHandle> {
    let path = info.hidraw_path.clone().ok_or_else(|| {
        HardwareError::NotFound("pas de hidraw_path — device non HID ou permission".to_string())
    })?;
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let serial = info.serial.clone();

    let join = std::thread::Builder::new()
        .name("maschine-hidraw".to_string())
        .spawn(move || {
            if let Err(e) = hidraw_loop(path, cfg, tx.clone(), stop2, serial) {
                let _ = tx.send(HardwareEvent::Error(format!("hidraw loop: {e}")));
            }
            let _ = tx.send(HardwareEvent::Disconnected);
        })
        .map_err(|e| HardwareError::Other(format!("spawn hidraw: {e}")))?;

    // petit délai pour laisser le thread envoyer Connected ou Error
    std::thread::sleep(Duration::from_millis(10));

    Ok(HidrawHandle {
        stop,
        join: Some(join),
    })
}

fn hidraw_loop(
    path: PathBuf,
    cfg: HidrawConfig,
    tx: Sender<HardwareEvent>,
    stop: Arc<AtomicBool>,
    serial: Option<String>,
) -> Result<()> {
    use std::os::unix::io::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .open(&path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                HardwareError::Permission(format!("{}: {}", path.display(), e))
            } else {
                HardwareError::Io(e)
            }
        })?;

    // non-blocking
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    unsafe {
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    let _ = tx.send(HardwareEvent::Connected { serial });

    let mut buf = [0u8; 128];
    let mut prev_pad_pressed = [false; 16];
    let mut prev_pad_velocity = [0u8; 16];
    // pour debounce boutons : map ButtonId -> Instant last change
    let mut last_poll = Instant::now();
    let mut tick_interval = Instant::now();

    while !stop.load(Ordering::Relaxed) {
        // poll avec timeout cfg.poll_interval
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let timeout_ms = cfg.poll_interval.as_millis() as i32;
        let ret = unsafe { libc::poll(&mut pfd as *mut _, 1, timeout_ms) };
        if ret < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(HardwareError::Io(e));
        }
        if ret == 0 {
            // timeout — heartbeat tick toutes les 100ms
            if tick_interval.elapsed() >= Duration::from_millis(100) {
                let ts = now_ns();
                let _ = tx.send(HardwareEvent::Tick { ts_ns: ts });
                tick_interval = Instant::now();
            }
            continue;
        }
        if pfd.revents & libc::POLLIN == 0 {
            if pfd.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                return Err(HardwareError::Other("hidraw hup/err".to_string()));
            }
            continue;
        }

        let n = match unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) } {
            n if n < 0 => {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    continue;
                }
                return Err(HardwareError::Io(e));
            }
            0 => {
                // EOF — device débranché
                return Err(HardwareError::Other(
                    "hidraw EOF — device débranché ?".to_string(),
                ));
            }
            n => n as usize,
        };

        let data = &buf[..n];
        if data.is_empty() {
            continue;
        }
        let ts = now_ns();

        match data[0] {
            0x01 => {
                match decode_report_01(data) {
                    Ok(inp) => {
                        for b in inp.buttons {
                            // debounce simple : on envoie tout pressé, le mapper fera le diff
                            let _ = tx.send(HardwareEvent::Button {
                                id: b.id,
                                pressed: b.pressed,
                                ts_ns: ts,
                            });
                        }
                        for enc in inp.encoders {
                            let _ = tx.send(HardwareEvent::Encoder(enc));
                        }
                        for ts_ev in inp.touchstrip {
                            let _ = tx.send(HardwareEvent::TouchStrip(ts_ev));
                        }
                        for (id, val) in inp.pots {
                            // map KnobId -> ButtonId placeholder
                            let bid = match id {
                                maschine_proto::hid_input::KnobId::MicGain => {
                                    maschine_proto::ButtonId::Unknown(99, 0)
                                }
                                maschine_proto::hid_input::KnobId::Headphone => {
                                    maschine_proto::ButtonId::Unknown(99, 1)
                                }
                                maschine_proto::hid_input::KnobId::Master => {
                                    maschine_proto::ButtonId::Unknown(99, 2)
                                }
                                _ => maschine_proto::ButtonId::Unknown(99, 3),
                            };
                            let _ = tx.send(HardwareEvent::Pot {
                                id: bid,
                                value: val,
                            });
                        }
                    }
                    Err(e) => {
                        log::warn!("decode 0x01 err: {e} data={:02x?}", &data[..n.min(16)]);
                    }
                }
            }
            0x02 => {
                match decode_report_02(data) {
                    Ok(pads) => {
                        // pour chaque pad brut, appliquer hystérésis + peak
                        // On garde un petit historique pour le pic vélocité sur 5 frames
                        for raw in pads {
                            if raw.pad as usize >= 16 {
                                continue;
                            }
                            let idx = raw.pad as usize;
                            let prev = prev_pad_pressed[idx];
                            let st = pad_to_state(raw, ts, prev);
                            // peak : si nouveau press, garder max velocity sur 3 frames
                            // Simplification : si press et prev false, velocity = raw.velocity
                            // si reste pressé, on prend max(prev, new) pour éviter chute
                            let velocity = if st.pressed && !prev {
                                st.velocity
                            } else if st.pressed && prev {
                                st.velocity.max(prev_pad_velocity[idx])
                            } else {
                                0
                            };
                            let mut out = st;
                            out.velocity = velocity;
                            prev_pad_pressed[idx] = st.pressed;
                            prev_pad_velocity[idx] = velocity;
                            if !st.pressed {
                                prev_pad_velocity[idx] = 0;
                            }
                            let _ = tx.send(HardwareEvent::Pad(out));
                        }
                        // pads relâchés mais plus présents dans le report ? Le firmware envoie seulement les actifs.
                        // On doit timeout les pads qui ne sont plus reportés — on le fait via tick 4ms sans update
                        // Pour MVP on ne timeout pas ici, le mapper gèrera le release par pression < PAD_OFF
                    }
                    Err(e) => {
                        log::warn!("decode 0x02 err: {e}");
                    }
                }
            }
            other => {
                log::debug!("hid unknown report 0x{other:02x} len {n}");
            }
        }

        // throttle
        let elapsed = last_poll.elapsed();
        if elapsed < cfg.poll_interval {
            std::thread::sleep(cfg.poll_interval - elapsed);
        }
        last_poll = Instant::now();
    }

    Ok(())
}

fn now_ns() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos() as u64
}

/// Dump synchrone bloquant pour debug CLI — lit N reports et affiche.
pub fn dump_hidraw_sync(path: PathBuf, duration: Duration) -> Result<()> {
    use std::os::unix::io::AsRawFd;
    let file = std::fs::OpenOptions::new().read(true).open(&path)?;
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    unsafe {
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    let start = Instant::now();
    let mut buf = [0u8; 128];
    println!("# hid dump {} for {:?}", path.display(), duration);
    while start.elapsed() < duration {
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ret = unsafe { libc::poll(&mut pfd as *mut _, 1, 200) };
        if ret <= 0 {
            continue;
        }
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n <= 0 {
            continue;
        }
        let n = n as usize;
        let data = &buf[..n];
        match data[0] {
            0x01 => {
                if let Ok(inp) = decode_report_01(data) {
                    if !inp.buttons.is_empty() {
                        println!("0x01 buttons: {:?}", inp.buttons);
                    }
                    if inp.encoders.iter().any(|e| e.value != 0) {
                        println!("0x01 encoders: {:?}", inp.encoders);
                    }
                }
            }
            0x02 => {
                if let Ok(pads) = decode_report_02(data) {
                    if !pads.is_empty() {
                        println!("0x02 pads: {:?}", pads);
                    }
                }
            }
            _ => println!("unknown {:02x?}", &data[..n.min(32)]),
        }
    }
    Ok(())
}
