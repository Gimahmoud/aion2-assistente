//! Tabela de opcodes. Muda a cada patch do jogo, por isso é dado e não código.

use serde::{Deserialize, Serialize};

/// Opcode como dois bytes na ordem em que aparecem no pacote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Op(pub u8, pub u8);

impl Op {
    pub fn key(self) -> u16 {
        u16::from(self.0) | (u16::from(self.1) << 8)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpcodeTable {
    /// Identificação da versão do cliente a que a tabela se refere.
    pub version: String,
    pub own_nickname: Op,
    pub other_nickname: Vec<Op>,
    pub summon: Op,
    pub damage: Op,
    pub dot: Op,
    pub battle_toggle: Op,
    pub remain_hp: Op,
    /// Crédito de abate: alvo, skill, quem matou e o nome dele. Visto no
    /// servidor global (SA, 10/2026); dá nome a jogadores sem pacote de nick.
    #[serde(default)]
    pub kill_credit: Option<Op>,
    /// Dados do grupo; a primeira entrada é o próprio personagem. Visto no
    /// servidor global (SA, 10/2026).
    #[serde(default)]
    pub party_info: Option<Op>,
    /// Pacote frequente que só fala do próprio personagem (começa com a
    /// entidade dele). Revela quem é o usuário mesmo quando o app abre com o
    /// jogo já em andamento. Visto no servidor global (SA, 10/2026).
    #[serde(default)]
    pub self_status: Option<Op>,
}

impl OpcodeTable {
    /// Tabela do TK Aion2-Dps-Meter v1.7.5 (servidor coreano, 06/2026).
    /// Precisa ser validada contra o tráfego do servidor global.
    pub fn kr_2026_06() -> Self {
        Self {
            version: "kr-2026-06 (TK v1.7.5)".into(),
            own_nickname: Op(0x33, 0x36),
            other_nickname: vec![Op(0x44, 0x36), Op(0x45, 0x36)],
            summon: Op(0x41, 0x36),
            damage: Op(0x04, 0x38),
            dot: Op(0x05, 0x38),
            battle_toggle: Op(0x21, 0x8d),
            remain_hp: Op(0x00, 0x8d),
            kill_credit: Some(Op(0x04, 0x8d)),
            party_info: Some(Op(0x00, 0x92)),
            self_status: Some(Op(0x4a, 0x36)),
        }
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

impl Default for OpcodeTable {
    fn default() -> Self {
        Self::kr_2026_06()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_json_matches_default() {
        let json = include_str!("../../../data/opcodes/kr-2026-06.json");
        assert_eq!(OpcodeTable::from_json(json).unwrap(), OpcodeTable::kr_2026_06());
    }
}
