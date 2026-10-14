//! Captura passiva do tráfego do jogo.
//!
//! Regra do projeto: este crate só **lê cópias** de pacotes. Ele nunca abre o
//! processo do jogo, nunca injeta, envia, altera ou bloqueia tráfego.

pub mod flows;
pub mod replay;
pub mod segment;

#[cfg(feature = "live")]
pub mod discover;
#[cfg(feature = "live")]
pub mod live;

pub use flows::{FlowStats, FlowTable, ServerFilter};
pub use segment::{parse_frame, LinkType, Segment};
