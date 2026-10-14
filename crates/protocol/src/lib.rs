//! Protocolo de rede do Aion 2, somente leitura.
//!
//! Pipeline: segmentos TCP -> [`tcp::TcpAligner`] -> [`framing::StreamAssembler`]
//! -> [`decode::Decoder`] -> [`decode::GameEvent`].
//!
//! Este crate nunca envia nem altera pacotes; só interpreta cópias.

pub mod decode;
pub mod framing;
pub mod opcodes;
pub mod tcp;
pub mod varint;

pub use decode::{DamageEvent, Decoder, EntityId, GameEvent};
pub use framing::{Message, StreamAssembler};
pub use opcodes::OpcodeTable;
pub use tcp::TcpAligner;
