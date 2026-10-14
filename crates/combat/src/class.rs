//! Classes jogáveis, deduzidas do código do personagem ou da skill.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    Gladiator,
    Templar,
    Assassin,
    Ranger,
    Sorcerer,
    Spiritmaster,
    Cleric,
    Chanter,
}

impl Class {
    /// Faixas de código de skill por classe (TK Aion2-Dps-Meter, MIT).
    pub fn from_skill(skill: u32) -> Option<Self> {
        Some(match skill / 1_000_000 {
            11 => Self::Gladiator,
            12 => Self::Templar,
            13 => Self::Assassin,
            14 => Self::Ranger,
            15 => Self::Sorcerer,
            16 => Self::Spiritmaster,
            17 => Self::Cleric,
            18 => Self::Chanter,
            _ => return None,
        })
    }

    /// Código de classe que vem junto do nome do personagem.
    pub fn from_job_code(code: u8) -> Option<Self> {
        Some(match code {
            5..=8 => Self::Gladiator,
            9..=12 => Self::Templar,
            13..=16 => Self::Ranger,
            17..=20 => Self::Assassin,
            21..=24 => Self::Spiritmaster,
            25..=28 => Self::Sorcerer,
            29..=32 => Self::Cleric,
            33..=36 => Self::Chanter,
            _ => return None,
        })
    }
}

/// Reduz o código bruto a 8 dígitos e zera o último, que varia com o nível
/// da skill. Sem catálogo de skills ainda, isso agrupa as variações.
pub fn normalize_skill(raw: u32) -> u32 {
    let mut x = raw;
    while x >= 100_000_000 {
        x /= 10;
    }
    x - x % 10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_normalization() {
        assert_eq!(normalize_skill(11_020_013), 11_020_010);
        assert_eq!(normalize_skill(1_402_000_100), 14_020_000);
        assert_eq!(Class::from_skill(normalize_skill(1_402_000_100)), Some(Class::Ranger));
    }
}
