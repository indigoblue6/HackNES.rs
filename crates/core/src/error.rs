//! エラー型の定義

use thiserror::Error;

/// NESエミュレータのエラー型
#[derive(Error, Debug)]
pub enum NesError {
    #[error("Invalid ROM format: {0}")]
    InvalidRom(String),

    #[error("Unsupported mapper: {0}")]
    UnsupportedMapper(u8),

    #[error("Invalid CPU instruction: {0:#04x}")]
    InvalidInstruction(u8),

    #[error("Memory access violation at address: {0:#06x}")]
    MemoryAccessViolation(u16),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

/// Result型のエイリアス
pub type Result<T> = std::result::Result<T, NesError>;
