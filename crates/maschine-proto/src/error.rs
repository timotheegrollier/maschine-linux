//! Erreurs du codec.

use thiserror::Error;

/// Erreur du protocole.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtoError {
    /// Report ID inattendu.
    #[error("report ID invalide: attendu 0x{expected:02x}, reçu 0x{got:02x}")]
    InvalidReportId {
        /// Attendu
        expected: u8,
        /// Reçu
        got: u8,
    },

    /// Taille invalide.
    #[error("taille invalide: attendu {expected}, reçu {got} pour report 0x{report_id:02x}")]
    InvalidSize {
        /// ID du report
        report_id: u8,
        /// Attendu
        expected: usize,
        /// Reçu
        got: usize,
    },

    /// Index hors borne.
    #[error("index hors borne: {what}={value} (max {max})")]
    OutOfBounds {
        /// Quoi
        what: &'static str,
        /// Valeur
        value: usize,
        /// Max
        max: usize,
    },

    /// Couleur inconnue.
    #[error("couleur index inconnue: {0}")]
    UnknownColor(u8),

    /// Display invalide.
    #[error("display invalide: {0}")]
    InvalidDisplay(u8),

    /// Header bulk invalide.
    #[error("header bulk invalide: {0}")]
    InvalidBulkHeader(&'static str),

    /// Données display invalides.
    #[error("données display invalides: {0}")]
    InvalidDisplayData(&'static str),
}

/// Résultat du codec.
pub type Result<T> = std::result::Result<T, ProtoError>;
