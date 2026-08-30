//! Décode les reports HID IN `0x01` et `0x02`.

use crate::error::{ProtoError, Result};

/// Identifiant bouton — exhaustif MK3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonId {
    // Row/col approximative, noms comme sérigraphie
    Shift,
    EncoderPush,
    FourWayUp,
    FourWayDown,
    FourWayLeft,
    FourWayRight,
    DisplayBtn(u8), // 1..8
    Group(u8),      // 0..7 = A..H
    Notes,
    Volume,
    Swing,
    Tempo,
    NoteRepeat,
    Lock,
    PadMode,
    Keyboard,
    Chords,
    Step,
    FixedVel,
    Scene,
    Pattern,
    Events,
    Variation,
    Duplicate,
    Select,
    Solo,
    Mute,
    Pitch,
    Mod,
    Perform,
    Restart,
    Erase,
    Tap,
    Follow,
    Play,
    Rec,
    Stop,
    Macro,
    Settings,
    Right,
    Sampling,
    Mixer,
    Plugin,
    ChannelMidi,
    Arranger,
    Browser,
    Left,
    FileSave,
    Auto,
    MainKnobTouch,
    KnobTouch(u8), // 1..8
    Pedal,
    Unknown(u8, u8), // byte, bit
}

/// État bouton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonEvent {
    /// Bouton
    pub id: ButtonId,
    /// Pressé
    pub pressed: bool,
}

/// Événement encodeur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncoderEvent {
    /// 0 = main, 1..8 = knobs
    pub id: u8,
    /// Valeur absolue 12-bit (0..4095, en pratique 0..1023) ou delta nibble pour main
    pub value: u16,
    /// Touché
    pub touched: bool,
}

/// Événement pad — pression + vélocité.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PadEvent {
    /// Index logique 0..15 (pad 1 = 0, bottom-left)
    pub pad: u8,
    /// Pression brute 0x4000..0x4FFD (0 si relâché)
    pub pressure: u16,
    /// Touché (pressure >= PAD_ON_THRESHOLD)
    pub touched: bool,
    /// Vélocité estimée 0..127 (pic sur 3-5 frames — ici instantané linéaire)
    pub velocity: u8,
}

/// Touchstrip 2 doigts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchStripEvent {
    /// Doigt 0 ou 1
    pub finger: u8,
    /// Position 0..1023
    pub position: u16,
    /// Pression
    pub pressure: u16,
    /// Touché
    pub touched: bool,
}

/// Identifiant knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnobId {
    /// Knob 1..8
    Knob(u8),
    /// Main encoder
    Main,
    /// Master pot
    Master,
    /// Headphone
    Headphone,
    /// Mic gain
    MicGain,
}

/// Tous les événements d'un poll HID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidInput {
    /// Boutons changés
    pub buttons: Vec<ButtonEvent>,
    /// Pads
    pub pads: Vec<PadEvent>,
    /// Encodeurs / knobs
    pub encoders: Vec<EncoderEvent>,
    /// Touchstrip
    pub touchstrip: Vec<TouchStripEvent>,
    /// Pots absolus (Mic/HP/Master)
    pub pots: Vec<(KnobId, u16)>,
}

// ---------------------------------------------------------------------------
// Helpers mapping bits → ButtonId
// ---------------------------------------------------------------------------

fn button_id_for(byte_idx: usize, bit: u8) -> ButtonId {
    match (byte_idx, bit) {
        (1, 0) => ButtonId::EncoderPush,
        (1, 1) => ButtonId::Pedal,
        (1, 2) => ButtonId::FourWayUp,
        (1, 3) => ButtonId::FourWayDown,
        (1, 4) => ButtonId::FourWayLeft,
        (1, 5) => ButtonId::FourWayRight,
        (1, 6) => ButtonId::Shift,
        (1, 7) => ButtonId::DisplayBtn(8),
        (2, 0) => ButtonId::Group(0),
        (2, 1) => ButtonId::Group(1),
        (2, 2) => ButtonId::Group(2),
        (2, 3) => ButtonId::Group(3),
        (2, 4) => ButtonId::Group(4),
        (2, 5) => ButtonId::Group(5),
        (2, 6) => ButtonId::Group(6),
        (2, 7) => ButtonId::Group(7),
        (3, 0) => ButtonId::Notes,
        (3, 1) => ButtonId::Volume,
        (3, 2) => ButtonId::Swing,
        (3, 3) => ButtonId::Tempo,
        (3, 4) => ButtonId::NoteRepeat,
        (3, 5) => ButtonId::Lock,
        (3, 6) => ButtonId::Unknown(3, 6),
        (3, 7) => ButtonId::Unknown(3, 7),
        (4, 0) => ButtonId::PadMode,
        (4, 1) => ButtonId::Keyboard,
        (4, 2) => ButtonId::Chords,
        (4, 3) => ButtonId::Step,
        (4, 4) => ButtonId::FixedVel,
        (4, 5) => ButtonId::Scene,
        (4, 6) => ButtonId::Pattern,
        (4, 7) => ButtonId::Events,
        (5, 0) => ButtonId::Unknown(5, 0),
        (5, 1) => ButtonId::Variation,
        (5, 2) => ButtonId::Duplicate,
        (5, 3) => ButtonId::Select,
        (5, 4) => ButtonId::Solo,
        (5, 5) => ButtonId::Mute,
        (5, 6) => ButtonId::Pitch,
        (5, 7) => ButtonId::Mod,
        (6, 0) => ButtonId::Perform,
        (6, 1) => ButtonId::Restart,
        (6, 2) => ButtonId::Erase,
        (6, 3) => ButtonId::Tap,
        (6, 4) => ButtonId::Follow,
        (6, 5) => ButtonId::Play,
        (6, 6) => ButtonId::Rec,
        (6, 7) => ButtonId::Stop,
        (7, 0) => ButtonId::Macro,
        (7, 1) => ButtonId::Settings,
        (7, 2) => ButtonId::Right,
        (7, 3) => ButtonId::Sampling,
        (7, 4) => ButtonId::Mixer,
        (7, 5) => ButtonId::Plugin,
        (7, 6) => ButtonId::Unknown(7, 6),
        (7, 7) => ButtonId::Unknown(7, 7),
        (8, 0) => ButtonId::ChannelMidi,
        (8, 1) => ButtonId::Arranger,
        (8, 2) => ButtonId::Browser,
        (8, 3) => ButtonId::Left,
        (8, 4) => ButtonId::FileSave,
        (8, 5) => ButtonId::Auto,
        (8, 6) => ButtonId::Unknown(8, 6),
        (8, 7) => ButtonId::Unknown(8, 7),
        (9, 0) => ButtonId::DisplayBtn(1),
        (9, 1) => ButtonId::DisplayBtn(2),
        (9, 2) => ButtonId::DisplayBtn(3),
        (9, 3) => ButtonId::DisplayBtn(4),
        (9, 4) => ButtonId::DisplayBtn(5),
        (9, 5) => ButtonId::DisplayBtn(6),
        (9, 6) => ButtonId::DisplayBtn(7),
        (9, 7) => ButtonId::MainKnobTouch,
        (10, 0) => ButtonId::KnobTouch(1),
        (10, 1) => ButtonId::KnobTouch(2),
        (10, 2) => ButtonId::KnobTouch(3),
        (10, 3) => ButtonId::KnobTouch(4),
        (10, 4) => ButtonId::KnobTouch(5),
        (10, 5) => ButtonId::KnobTouch(6),
        (10, 6) => ButtonId::KnobTouch(7),
        (10, 7) => ButtonId::KnobTouch(8),
        _ => ButtonId::Unknown(byte_idx as u8, bit),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Décode un report `0x01` (boutons + knobs + touchstrip).
/// `data` doit inclure le byte d'ID (`0x01`) en position 0.
pub fn decode_report_01(data: &[u8]) -> Result<HidInput> {
    if data.is_empty() {
        return Err(ProtoError::InvalidSize {
            report_id: 0x01,
            expected: crate::consts::HID_REPORT_01_SIZE + 1,
            got: 0,
        });
    }
    if data[0] != 0x01 {
        return Err(ProtoError::InvalidReportId {
            expected: 0x01,
            got: data[0],
        });
    }
    if data.len() < crate::consts::HID_REPORT_01_SIZE + 1 {
        return Err(ProtoError::InvalidSize {
            report_id: 0x01,
            expected: crate::consts::HID_REPORT_01_SIZE + 1,
            got: data.len(),
        });
    }

    let mut buttons = Vec::with_capacity(16);
    // bytes 1..10 = bitmasks
    #[allow(clippy::needless_range_loop)]
    for byte_idx in 1..=10 {
        let b = data[byte_idx];
        for bit in 0..8 {
            let pressed = (b >> bit) & 1 == 1;
            // On émet seulement les pressés ; le caller fera le diff avec l'état précédent
            // Pour simplifier ici on émet tout pressé (pas les relâchés) — le hardware layer gère le diff
            if pressed {
                let id = button_id_for(byte_idx, bit);
                buttons.push(ButtonEvent { id, pressed: true });
            }
        }
    }

    let mut encoders = Vec::new();
    // byte 11 : main encoder nibble
    let main_val = (data[11] & 0x0F) as u16;
    encoders.push(EncoderEvent {
        id: 0,
        value: main_val,
        touched: false,
    });
    // bytes 12..27 : knobs 1..8 absolute 10-bit little endian (2 bytes each, mais doc dit split)
    // Simplification : chaque knob sur 2 bytes LE 10-bit
    for i in 0..8 {
        let lo = data[12 + i * 2] as u16;
        let hi = data[12 + i * 2 + 1] as u16;
        let val = (hi << 8) | lo;
        let val = val & 0x3FF; // 10-bit
        encoders.push(EncoderEvent {
            id: (i + 1) as u8,
            value: val,
            touched: false,
        });
    }

    // touchstrip 28..35 : 2 fingers × 4 bytes → pos/pressure (approximation)
    let mut touchstrip = Vec::new();
    for finger in 0..2 {
        let base = 28 + finger * 4;
        let p0 = data[base] as u16;
        let p1 = data[base + 1] as u16;
        let v = (p1 << 8) | p0;
        let touched = v != 0 && v != 0xFFFF;
        touchstrip.push(TouchStripEvent {
            finger: finger as u8,
            position: v & 0x3FF,
            pressure: (v >> 10) & 0x3F,
            touched,
        });
    }

    // pots 36..41 : Mic Gain / HP / Master (3 × 2 bytes)
    let mut pots = Vec::new();
    let pot_ids = [KnobId::MicGain, KnobId::Headphone, KnobId::Master];
    for (idx, id) in pot_ids.iter().enumerate() {
        let base = 36 + idx * 2;
        if base + 1 < data.len() {
            let lo = data[base] as u16;
            let hi = data[base + 1] as u16;
            let v = (hi << 8) | lo;
            pots.push((*id, v & 0x3FF));
        }
    }

    // Injecte touch flags depuis bytes 9-10
    for ev in &mut encoders {
        if ev.id == 0 {
            // main touch = byte 9 bit 7
            ev.touched = (data[9] >> 7) & 1 == 1;
        } else if (1..=8).contains(&ev.id) {
            let bit = ev.id - 1;
            // byte 10 bits 0..7 = knob touch 1..8
            ev.touched = (data[10] >> bit) & 1 == 1;
        }
    }

    Ok(HidInput {
        buttons,
        pads: Vec::new(),
        encoders,
        touchstrip,
        pots,
    })
}

/// Décode un report `0x02` (pads pression).
/// `data` inclut l'ID `0x02` en position 0.
pub fn decode_report_02(data: &[u8]) -> Result<Vec<PadEvent>> {
    if data.is_empty() {
        return Err(ProtoError::InvalidSize {
            report_id: 0x02,
            expected: crate::consts::HID_REPORT_02_SIZE + 1,
            got: 0,
        });
    }
    if data[0] != 0x02 {
        return Err(ProtoError::InvalidReportId {
            expected: 0x02,
            got: data[0],
        });
    }
    if data.len() < crate::consts::HID_REPORT_02_SIZE + 1 {
        return Err(ProtoError::InvalidSize {
            report_id: 0x02,
            expected: crate::consts::HID_REPORT_02_SIZE + 1,
            got: data.len(),
        });
    }

    let mut pads = Vec::new();
    // 16 slots × 3 bytes, certains implémentent 21 slots — on parse tout ce qu'on a
    let slots = (data.len() - 1) / 3;
    for i in 0..slots.min(21) {
        let base = 1 + i * 3;
        let pad_idx = data[base];
        if pad_idx == 0xFF || pad_idx > 15 {
            // slot inutilisé (padding)
            continue;
        }
        let hi = data[base + 1] as u16;
        let lo = data[base + 2] as u16;
        let raw = (hi << 8) | lo;
        // high nibble 0x4 = valide
        if (raw & 0xF000) != 0x4000 {
            continue;
        }
        let pressure = raw;
        let touched = pressure >= crate::consts::PAD_ON_THRESHOLD;
        // velocity linéaire simple (remplacé par pic 3-5 frames dans hardware layer)
        let velocity = if touched {
            let norm = (pressure - 0x4000) as f32 / (0x0FFD as f32); // 0x4000..0x4FFD ≈ 0..4093
            (norm * 127.0).clamp(1.0, 127.0) as u8
        } else {
            0
        };
        // Remap hardware order top-right→bottom-left vers logique 0..15 bottom-left→top-right
        // Hardware: 13-16,9-12,5-8,1-4 → logique: 0-3 bottom, 4-7, 8-11, 12-15 top
        // Pour MVP on garde hardware idx et on laisse maschine-hardware remapper
        pads.push(PadEvent {
            pad: pad_idx,
            pressure,
            touched,
            velocity,
        });
    }
    Ok(pads)
}

/// Décode un buffer HID brut (dispatch 0x01 / 0x02) — helper pour le thread hid.
pub fn decode_hid_report(data: &[u8]) -> Result<HidInput> {
    if data.is_empty() {
        return Err(ProtoError::InvalidSize {
            report_id: 0,
            expected: 1,
            got: 0,
        });
    }
    match data[0] {
        0x01 => decode_report_01(data),
        0x02 => {
            let pads = decode_report_02(data)?;
            Ok(HidInput {
                buttons: Vec::new(),
                pads,
                encoders: Vec::new(),
                touchstrip: Vec::new(),
                pots: Vec::new(),
            })
        }
        x => Err(ProtoError::InvalidReportId {
            expected: 0x01,
            got: x,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_report_01() -> Vec<u8> {
        let mut v = vec![0u8; 43];
        v[0] = 0x01;
        v[1] = 1 << 6; // Shift
        v[2] = 1 << 0; // Group A
        v[11] = 0x05; // main encoder 5
        v
    }

    #[test]
    fn decode_01_shift_and_group() {
        let r = fake_report_01();
        let inp = decode_report_01(&r).unwrap();
        assert!(inp.buttons.iter().any(|b| b.id == ButtonId::Shift));
        assert!(inp.buttons.iter().any(|b| b.id == ButtonId::Group(0)));
        assert_eq!(inp.encoders[0].value, 5);
    }

    #[test]
    fn decode_01_wrong_id() {
        let mut r = fake_report_01();
        r[0] = 0x02;
        assert!(decode_report_01(&r).is_err());
    }

    #[test]
    fn decode_02_pads() {
        let mut v = vec![0u8; 65];
        v[0] = 0x02;
        v[1] = 5; // pad 5
        v[2] = 0x40;
        v[3] = 0x80; // 0x4080
        let pads = decode_report_02(&v).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].pad, 5);
        assert!(pads[0].touched);
        assert!(pads[0].velocity > 0);
        // slot vide
        v[1] = 0xFF;
        let pads2 = decode_report_02(&v).unwrap();
        assert_eq!(pads2.len(), 0);
    }

    #[test]
    fn decode_02_off_threshold() {
        let mut v = vec![0u8; 65];
        v[0] = 0x02;
        v[1] = 2;
        v[2] = 0x40;
        v[3] = 0x02; // 0x4002 < PAD_OFF
        let pads = decode_report_02(&v).unwrap();
        assert_eq!(pads.len(), 1);
        assert!(!pads[0].touched);
    }
}
