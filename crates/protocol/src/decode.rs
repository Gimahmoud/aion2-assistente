//! Decodificadores por opcode. Cada um devolve eventos de domínio ou nada.
//!
//! As estruturas foram portadas do `StreamProcessor` do TK Aion2-Dps-Meter
//! (MIT) e descrevem o cliente coreano de 06/2026. Campos marcados como
//! "desconhecido" são pulados sem interpretação.

use serde::Serialize;

use crate::framing::Message;
use crate::opcodes::OpcodeTable;
use crate::varint::{self, u16_le, u32_le};

/// ID de entidade no servidor (jogador, monstro, invocação).
pub type EntityId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SpecialFlags {
    pub back: bool,
    pub parry: bool,
    pub perfect: bool,
    pub double: bool,
    pub endure: bool,
    pub restoration: bool,
}

impl SpecialFlags {
    const NONE: Self = Self {
        back: false,
        parry: false,
        perfect: false,
        double: false,
        endure: false,
        restoration: false,
    };

    fn from_byte(b: u8) -> Self {
        Self {
            back: b & 0x01 != 0,
            parry: b & 0x04 != 0,
            perfect: b & 0x08 != 0,
            double: b & 0x10 != 0,
            endure: b & 0x20 != 0,
            restoration: b & 0x40 != 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DamageEvent {
    pub actor: EntityId,
    pub target: EntityId,
    /// Código da skill como veio no pacote. A normalização (tirar o nível
    /// da skill dos últimos dígitos) é feita no motor de combate.
    pub skill_raw: u32,
    pub damage: u32,
    /// Quantidade de golpes informada no pacote (0 quando não vem). O dano
    /// em `damage` já é o total; isto é só informativo.
    pub hits: u8,
    pub dot: bool,
    pub specials: SpecialFlags,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameEvent {
    Damage(DamageEvent),
    /// Personagem do próprio usuário (enviado ao entrar no mundo).
    OwnCharacter {
        entity: EntityId,
        name: String,
        server: Option<u16>,
        job_code: Option<u8>,
    },
    OtherCharacter {
        entity: EntityId,
        name: String,
        job_code: Option<u8>,
    },
    MobSpawn {
        entity: EntityId,
        mob_code: u32,
    },
    SummonOwner {
        summon: EntityId,
        owner: EntityId,
    },
    BattleToggle {
        entity: EntityId,
        in_battle: bool,
    },
    MobHp {
        entity: EntityId,
        hp: u32,
    },
}

pub struct Decoder {
    table: OpcodeTable,
    pub unknown_opcodes: std::collections::HashMap<u16, u64>,
}

impl Decoder {
    pub fn new(table: OpcodeTable) -> Self {
        Self {
            table,
            unknown_opcodes: Default::default(),
        }
    }

    pub fn table(&self) -> &OpcodeTable {
        &self.table
    }

    pub fn decode(&mut self, msg: &Message) -> Vec<GameEvent> {
        let Some(op) = msg.opcode() else { return vec![] };
        let t = &self.table;
        let ev = if op == t.damage.key() {
            parse_damage(msg).map(GameEvent::Damage)
        } else if op == t.dot.key() {
            parse_dot(msg).map(GameEvent::Damage)
        } else if op == t.own_nickname.key() {
            parse_own_nickname(msg)
        } else if t.other_nickname.iter().any(|o| o.key() == op) {
            parse_other_nickname(msg)
        } else if op == t.summon.key() {
            return parse_summon(msg);
        } else if op == t.battle_toggle.key() {
            parse_battle_toggle(msg)
        } else if op == t.remain_hp.key() {
            parse_remain_hp(msg)
        } else if t.kill_credit.is_some_and(|o| o.key() == op) {
            parse_kill_credit(msg)
        } else if t.party_info.is_some_and(|o| o.key() == op) {
            parse_party_self(msg)
        } else {
            *self.unknown_opcodes.entry(op).or_default() += 1;
            None
        };
        ev.into_iter().collect()
    }
}

/// Cursor simples sobre os bytes da mensagem.
struct Cur<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Cur<'a> {
    fn after_opcode(msg: &'a Message) -> Self {
        Self {
            b: &msg.bytes,
            pos: msg.opcode_offset() + 2,
        }
    }
    fn varint(&mut self) -> Option<u32> {
        let v = varint::read(self.b, self.pos)?;
        self.pos += v.len;
        Some(v.value)
    }
    fn byte(&mut self) -> Option<u8> {
        let v = *self.b.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }
    fn u32(&mut self) -> Option<u32> {
        let v = u32_le(self.b, self.pos)?;
        self.pos += 4;
        Some(v)
    }
    fn has(&self, n: usize) -> bool {
        self.pos + n <= self.b.len()
    }
}

fn parse_damage(msg: &Message) -> Option<DamageEvent> {
    if msg.bytes.first() == Some(&0x20) {
        return None;
    }
    let mut c = Cur::after_opcode(msg);
    let target = c.varint()?;
    let switch = c.varint()?;
    let _flag = c.varint()?;
    let actor = c.varint()?;
    if !c.has(6) {
        return None;
    }
    let skill_raw = u32_le(c.b, c.pos)?;
    c.pos += 5; // 4 bytes de skill + 1 desconhecido
    let _kind = c.varint()?;

    // Tamanho do bloco de flags depende dos 4 bits baixos de `switch`.
    // O TK (KR 06/2026) usava 10 para o tipo 6; no servidor global (SA,
    // 10/2026) são 11: [flags especiais][00][01|02][4 bytes]. Conferido em
    // gravações reais; 5 e 7 ainda não apareceram e seguem os valores do TK.
    let block = match switch & 0x0f {
        4 => 8,
        5 => 12,
        6 => 11,
        7 => 14,
        _ => return None,
    };
    if !c.has(block) {
        return None;
    }
    let specials = if block >= 10 {
        SpecialFlags::from_byte(c.b[c.pos])
    } else {
        SpecialFlags::NONE
    };
    if specials.restoration {
        c.pos += 2;
    }
    c.pos += block;

    let _unknown = c.varint()?;
    let damage = c.varint()?;
    // Com o bit 0x20 vem uma lista [n][valor]{n}. Os valores não são o dano
    // de cada golpe (são pequenos e não somam o total), então o dano acima já
    // é o total e não deve ser multiplicado.
    let hits = if switch & 0x20 != 0 { skip_hit_list(&mut c) } else { 0 };

    if actor == target {
        return None;
    }
    Some(DamageEvent {
        actor,
        target,
        skill_raw,
        damage,
        hits,
        dot: false,
        specials,
    })
}

/// Pula `[n][valor]{n}` e devolve `n`.
fn skip_hit_list(c: &mut Cur) -> u8 {
    let Some(n) = c.byte() else { return 0 };
    for _ in 0..n {
        if c.varint().is_none() {
            return 0;
        }
    }
    n
}

fn parse_dot(msg: &Message) -> Option<DamageEvent> {
    let mut c = Cur::after_opcode(msg);
    let target = c.varint()?;
    let flags = c.byte()?;
    if flags & 0x02 == 0 {
        return None;
    }
    let actor = c.varint()?;
    if actor == target {
        return None;
    }
    let _unknown = c.varint()?;
    let skill_raw = c.u32()?;
    if !c.has(1) {
        return None;
    }
    let damage = c.varint()?;
    Some(DamageEvent {
        actor,
        target,
        skill_raw,
        damage,
        hits: 0,
        dot: true,
        specials: SpecialFlags::NONE,
    })
}

fn read_name(b: &[u8], pos: usize) -> Option<(String, usize)> {
    let len = varint::read(b, pos)?;
    if len.value == 0 || len.value > 71 {
        return None;
    }
    let start = pos + len.len;
    let end = start + len.value as usize;
    let s = std::str::from_utf8(b.get(start..end)?).ok()?;
    is_valid_name(s).then(|| (s.to_owned(), end))
}

/// Nomes de personagem: letras (qualquer alfabeto) e dígitos, com ao menos
/// uma letra. O TK aceitava só hangul e latim; o servidor global tem acentos.
fn is_valid_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(char::is_alphanumeric) && s.chars().any(char::is_alphabetic)
}

fn parse_own_nickname(msg: &Message) -> Option<GameEvent> {
    let mut c = Cur::after_opcode(msg);
    let entity = c.varint()?;
    let window = c.b.get(c.pos..(c.pos + 10).min(c.b.len()))?;
    let sep = window.iter().position(|&x| x == 0x07)?;
    let (name, end) = read_name(c.b, c.pos + sep + 1)?;
    let server = u16_le(c.b, end);
    let job_code = c.b.get(end + 2).copied();
    Some(GameEvent::OwnCharacter {
        entity,
        name,
        server,
        job_code,
    })
}

fn parse_other_nickname(msg: &Message) -> Option<GameEvent> {
    let mut c = Cur::after_opcode(msg);
    let entity = c.varint()?;
    c.varint()?;
    c.varint()?;
    c.pos += 1;
    let base = c.pos;
    let (name, end) = (0..5).find_map(|i| read_name(c.b, base + i))?;
    let job_code = c.b.get(end).copied();
    Some(GameEvent::OtherCharacter { entity, name, job_code })
}

/// `[alvo][skill u32][quem matou][servidor u16][nome][guilda]...`
fn parse_kill_credit(msg: &Message) -> Option<GameEvent> {
    let mut c = Cur::after_opcode(msg);
    c.varint()?;
    c.u32()?;
    let entity = c.varint()?;
    c.pos += 2;
    let (name, _) = read_name(c.b, c.pos)?;
    Some(GameEvent::OtherCharacter {
        entity,
        name,
        job_code: None,
    })
}

/// Lista do grupo. A primeira entrada é o próprio personagem:
/// `[entidade u32][servidor u16][servidor u16][36 "uuid"][6 bytes][servidor u16][nome]`.
fn parse_party_self(msg: &Message) -> Option<GameEvent> {
    let b = &msg.bytes;
    let first = msg.opcode_offset() + 2 + 8;
    let uuid = (first..b.len().saturating_sub(37)).find(|&i| b[i] == 36 && is_uuid(&b[i + 1..i + 37]))?;
    let entity = u32_le(b, uuid - 8)?;
    let server = u16_le(b, uuid - 4)?;
    if !(1..0x0100_0000).contains(&entity) {
        return None;
    }
    let (name, _) = read_name(b, uuid + 37 + 8)?;
    Some(GameEvent::OwnCharacter {
        entity,
        name,
        server: Some(server),
        job_code: None,
    })
}

fn is_uuid(s: &[u8]) -> bool {
    s.len() == 36
        && s.iter().enumerate().all(|(i, &c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn parse_summon(msg: &Message) -> Vec<GameEvent> {
    let mut out = Vec::new();
    let mut c = Cur::after_opcode(msg);
    let Some(summon) = c.varint() else { return out };
    let b = &msg.bytes;

    let marker = find(b, &[0x00, 0x40, 0x02]).or_else(|| find(b, &[0x00, 0x00, 0x02]));
    if let Some(i) = marker.filter(|&i| i >= 3) {
        let mob_code = u32::from(b[i - 3]) | (u32::from(b[i - 2]) << 8) | (u32::from(b[i - 1]) << 16);
        out.push(GameEvent::MobSpawn {
            entity: summon,
            mob_code,
        });
    }

    if let Some(key) = find(b, &[0xff; 8]) {
        if let Some(op) = find(&b[key + 8..], &[0x07, 0x02, 0x06]) {
            if let Some(owner) = u16_le(b, key + op + 11) {
                out.push(GameEvent::SummonOwner {
                    summon,
                    owner: u32::from(owner),
                });
            }
        }
    }
    out
}

fn parse_battle_toggle(msg: &Message) -> Option<GameEvent> {
    let mut c = Cur::after_opcode(msg);
    let entity = c.varint()?;
    c.varint()?;
    match c.varint()? {
        1 => Some(GameEvent::BattleToggle {
            entity,
            in_battle: true,
        }),
        0 => Some(GameEvent::BattleToggle {
            entity,
            in_battle: false,
        }),
        _ => None,
    }
}

fn parse_remain_hp(msg: &Message) -> Option<GameEvent> {
    let mut c = Cur::after_opcode(msg);
    let entity = c.varint()?;
    c.varint()?;
    c.varint()?;
    c.varint()?;
    let hp = c.u32()?;
    Some(GameEvent::MobHp { entity, hp })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framing::{build_frame, StreamAssembler};
    use crate::varint::encode;

    fn v(x: u32, out: &mut Vec<u8>) {
        encode(x, out);
    }

    /// Monta um pacote de dano no layout esperado pelo decodificador.
    fn damage_body(target: u32, actor: u32, skill: u32, dmg: u32, switch: u32, specials: u8) -> Vec<u8> {
        let mut b = vec![0x04, 0x38];
        v(target, &mut b);
        v(switch, &mut b);
        v(0, &mut b); // flag
        v(actor, &mut b);
        b.extend_from_slice(&skill.to_le_bytes());
        b.push(0); // byte desconhecido
        v(1, &mut b); // tipo
        let block = match switch & 0x0f {
            4 => 8,
            5 => 12,
            6 => 11,
            _ => 14,
        };
        let mut flags = vec![0u8; block];
        flags[0] = specials;
        b.extend_from_slice(&flags);
        v(0, &mut b); // desconhecido
        v(dmg, &mut b);
        b.push(0); // sem multi-hit
        b.extend_from_slice(&[0, 0]);
        b
    }

    fn decode_one(frame: Vec<u8>) -> Vec<GameEvent> {
        let mut asm = StreamAssembler::new();
        let msgs = asm.push(&frame);
        assert_eq!(msgs.len(), 1);
        Decoder::new(OpcodeTable::default()).decode(&msgs[0])
    }

    #[test]
    fn decodes_damage() {
        let ev = decode_one(build_frame(
            None,
            &damage_body(500, 42, 11020010, 12_345, 6, 0x08 | 0x01),
        ));
        let GameEvent::Damage(d) = &ev[0] else { panic!("{ev:?}") };
        assert_eq!((d.actor, d.target, d.skill_raw, d.damage), (42, 500, 11020010, 12_345));
        assert!(d.specials.perfect && d.specials.back && !d.dot);
    }

    #[test]
    fn decodes_damage_with_extra_flag_byte() {
        let ev = decode_one(build_frame(Some(0xf1), &damage_body(7, 8, 13010000, 99, 4, 0)));
        assert!(matches!(&ev[0], GameEvent::Damage(d) if d.damage == 99 && d.actor == 8));
    }

    #[test]
    fn ignores_self_damage_and_bad_switch() {
        assert!(decode_one(build_frame(None, &damage_body(5, 5, 1, 1, 4, 0))).is_empty());
        assert!(decode_one(build_frame(None, &damage_body(5, 6, 1, 1, 3, 0))).is_empty());
    }

    #[test]
    fn decodes_dot() {
        let mut b = vec![0x05, 0x38];
        v(900, &mut b);
        b.push(0x02);
        v(31, &mut b);
        v(0, &mut b);
        b.extend_from_slice(&1_402_000_100u32.to_le_bytes());
        v(777, &mut b);
        b.push(0);
        let ev = decode_one(build_frame(None, &b));
        assert!(matches!(&ev[0], GameEvent::Damage(d) if d.dot && d.damage == 777 && d.actor == 31));
    }

    #[test]
    fn decodes_own_nickname() {
        let mut b = vec![0x33, 0x36];
        v(1234, &mut b);
        b.extend_from_slice(&[0x01, 0x07]);
        let name = "Lumière";
        v(name.len() as u32, &mut b);
        b.extend_from_slice(name.as_bytes());
        b.extend_from_slice(&2005u16.to_le_bytes());
        b.push(26);
        let ev = decode_one(build_frame(None, &b));
        assert_eq!(
            ev,
            vec![GameEvent::OwnCharacter {
                entity: 1234,
                name: name.into(),
                server: Some(2005),
                job_code: Some(26)
            }]
        );
    }

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|x| u8::from_str_radix(x, 16).unwrap())
            .collect()
    }

    /// Pacotes reais do servidor SA (10/2026), capturados pelo app.
    #[test]
    fn decodes_real_sa_damage() {
        // Tipo 6: o bloco tem 11 bytes. Antes lia o campo `f7 6b` (13815)
        // como dano.
        let ev = decode_one(hex(
            "24 04 38 91 c6 01 06 00 e6 44 22 d0 da 00 1c 03 00 00 02 43 26 79 55 01 00 00 00 f7 6b ec 10 01 00",
        ));
        let GameEvent::Damage(d) = &ev[0] else { panic!("{ev:?}") };
        assert_eq!(
            (d.actor, d.target, d.skill_raw, d.damage),
            (8806, 25361, 14340130, 2156)
        );

        // Bit 0x20: lista de 4 golpes depois do dano; o dano não é multiplicado.
        let ev = decode_one(hex(
            "29 04 38 91 c6 01 26 00 e6 44 22 d0 da 00 19 02 04 00 02 4d 26 79 55 02 00 00 00 f7 6b ec 0b 04 21 21 21 21 02 00",
        ));
        let GameEvent::Damage(d) = &ev[0] else { panic!("{ev:?}") };
        assert_eq!((d.damage, d.hits), (1516, 4));

        // Tipo 4 com bit 0x20.
        let ev = decode_one(hex(
            "26 04 38 91 c6 01 24 00 e6 44 22 d0 da 00 19 02 43 26 79 55 01 00 00 00 f7 6b 8e 0b 04 1f 1f 1f 1f 01 00",
        ));
        assert!(matches!(&ev[0], GameEvent::Damage(d) if d.damage == 1422));
    }

    #[test]
    fn decodes_real_sa_kill_credit() {
        let ev = decode_one(hex(
            "30 04 8d 91 c6 01 22 d0 da 00 e6 44 79 05 09 47 69 6d 61 68 6d 6f 75 64 0b 4c 69 67 61 63 72 69 74 69 63 61 01 00 00 00 00 00 00 01 00",
        ));
        assert_eq!(
            ev,
            vec![GameEvent::OtherCharacter {
                entity: 8806,
                name: "Gimahmoud".into(),
                job_code: None
            }]
        );
    }

    #[test]
    fn decodes_real_sa_party_self() {
        let mut b = hex("00 92 00 48 4b 01 00 dc ef 25 9a 9d 6d 03 00 00 00 64 09 05 00 00 00 02 01 00 02 02 03 02 66 22 00 00 79 05 79 05 24");
        b.extend_from_slice(b"26F7D912-2C2A-4482-AB62-A6E71D2A4306");
        b.extend(hex(
            "5d ce 04 00 00 00 79 05 09 47 69 6d 61 68 6d 6f 75 64 0e 00 00 00 93 92 01",
        ));
        let ev = decode_one(build_frame(None, &b));
        assert_eq!(
            ev,
            vec![GameEvent::OwnCharacter {
                entity: 8806,
                name: "Gimahmoud".into(),
                server: Some(1401),
                job_code: None
            }]
        );
    }

    #[test]
    fn counts_unknown_opcodes() {
        let mut asm = StreamAssembler::new();
        let msgs = asm.push(&build_frame(None, &[0x99, 0x99, 1, 2, 3]));
        let mut d = Decoder::new(OpcodeTable::default());
        assert!(d.decode(&msgs[0]).is_empty());
        assert_eq!(d.unknown_opcodes.get(&0x9999), Some(&1));
    }
}
