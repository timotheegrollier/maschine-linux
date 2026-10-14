//! Événements haut niveau émis par le thread HID.

use maschine_proto::hid_input::{ButtonEvent, EncoderEvent, PadEvent, TouchStripEvent};
use maschine_proto::ButtonId;
use serde::{Deserialize, Serialize};

/// État pad avec debounce + vélocité pic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PadState {
    /// Index 0..15
    pub pad: u8,
    /// Pressé (après hystérésis)
    pub pressed: bool,
    /// Vélocité 0..127 (pic au press)
    pub velocity: u8,
    /// Pression brute
    pub pressure: u16,
    /// Timestamp ns (monotonic)
    pub ts_ns: u64,
}

/// Événement hardware normalisé — envoyé du hid thread vers engine/mapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardwareEvent {
    /// Bouton press/release (avec debounce)
    Button {
        /// Bouton
        id: ButtonId,
        /// Pressé
        pressed: bool,
        /// Timestamp ns
        ts_ns: u64,
    },
    /// Pad hit/release
    Pad(PadState),
    /// Encodeur / knob absolute
    Encoder(EncoderEvent),
    /// Touchstrip
    TouchStrip(TouchStripEvent),
    /// Pot absolu (Mic/HP/Master)
    Pot {
        /// Id
        id: ButtonId,
        /// Valeur 0..1023
        value: u16,
    },
    /// Tick — heartbeat pour timeouts
    Tick {
        /// ns
        ts_ns: u64,
    },
    /// Erreur / déconnexion
    Error(String),
    /// Connexion établie
    Connected {
        /// Serial
        serial: Option<String>,
    },
    /// Déconnexion
    Disconnected,
}

impl HardwareEvent {
    /// Vrai si c'est un pad press.
    #[must_use]
    pub fn is_pad_press(&self) -> bool {
        matches!(self, Self::Pad(p) if p.pressed)
    }

    /// Vrai si c'est un pad release.
    #[must_use]
    pub fn is_pad_release(&self) -> bool {
        matches!(self, Self::Pad(p) if !p.pressed)
    }
}

/// Helpers pour ButtonEvent -> HardwareEvent.
pub fn button_to_event(ev: ButtonEvent, ts_ns: u64) -> HardwareEvent {
    HardwareEvent::Button {
        id: ev.id,
        pressed: ev.pressed,
        ts_ns,
    }
}

/// Convertit PadEvent brut en PadState avec hystérésis (le caller gère l'historique).
/// Cette fonction ne fait que mapper — l'hystérésis 0x4010/0x4005 est appliquée ici.
pub fn pad_to_state(raw: PadEvent, ts_ns: u64, prev_pressed: bool) -> PadState {
    let pressed = if prev_pressed {
        raw.pressure >= maschine_proto::consts::pad_off_threshold()
    } else {
        raw.pressure >= maschine_proto::consts::pad_on_threshold()
    };
    // Si on vient de presser, velocity = raw.velocity, sinon on garde 0 en release
    // Le mapper peut lisser sur plusieurs frames pour le peak
    PadState {
        pad: raw.pad,
        pressed,
        velocity: if pressed { raw.velocity.max(1) } else { 0 },
        pressure: raw.pressure,
        ts_ns,
    }
}
