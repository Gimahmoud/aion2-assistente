//! Agenda de eventos: regras fixas ou por intervalo no relógio do servidor.

use serde::{Deserialize, Serialize};

use crate::clock::{ServerClock, TimeOfDay, DAY_MS, MINUTE_MS};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Recurrence {
    /// Horários do dia; `dias` vazio = todos os dias (0 = domingo).
    Fixo {
        horarios: Vec<TimeOfDay>,
        #[serde(default)]
        dias: Vec<u8>,
    },
    /// A cada `a_cada_min` minutos a partir de `offset_min`, recomeçando à meia-noite do servidor.
    Intervalo {
        a_cada_min: u32,
        #[serde(default)]
        offset_min: u32,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventRule {
    pub id: String,
    pub nome: String,
    #[serde(default)]
    pub descricao: Option<String>,
    #[serde(flatten)]
    pub recorrencia: Recurrence,
    #[serde(default)]
    pub duracao_min: Option<u32>,
    /// Antecedência padrão do alerta.
    #[serde(default = "default_aviso")]
    pub aviso_min: u32,
    /// Se o alerta já vem ligado na primeira vez.
    #[serde(default = "default_true")]
    pub alerta_padrao: bool,
}

fn default_aviso() -> u32 {
    5
}
fn default_true() -> bool {
    true
}

/// Uma ocorrência de evento, em UTC.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Occurrence {
    pub start_ms: i64,
    pub end_ms: Option<i64>,
}

impl EventRule {
    /// Inícios (UTC) em `[from_ms, to_ms)`, em ordem.
    pub fn starts_between(&self, clock: &ServerClock, from_ms: i64, to_ms: i64) -> Vec<i64> {
        let mut out = Vec::new();
        if to_ms <= from_ms {
            return out;
        }
        let first_day = clock.to_server(from_ms).div_euclid(DAY_MS) - 1;
        let last_day = clock.to_server(to_ms).div_euclid(DAY_MS);
        for day in first_day..=last_day {
            let midnight = day * DAY_MS;
            let mut push = |minute: u32| {
                let t = clock.to_utc(midnight + minute as i64 * MINUTE_MS);
                if t >= from_ms && t < to_ms {
                    out.push(t);
                }
            };
            match &self.recorrencia {
                Recurrence::Fixo { horarios, dias } => {
                    if dias.is_empty() || dias.contains(&ServerClock::weekday(midnight)) {
                        horarios.iter().for_each(|h| push(h.0));
                    }
                }
                Recurrence::Intervalo { a_cada_min, offset_min } => {
                    let step = (*a_cada_min).max(1);
                    let mut m = *offset_min;
                    while m < 24 * 60 {
                        push(m);
                        m += step;
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Ocorrências em andamento ou que começam até `to_ms`.
    pub fn occurrences(&self, clock: &ServerClock, now_ms: i64, to_ms: i64) -> Vec<Occurrence> {
        let dur = self.duracao_min.map(|d| d as i64 * MINUTE_MS);
        let from = now_ms - dur.unwrap_or(0);
        self.starts_between(clock, from, to_ms)
            .into_iter()
            .map(|start_ms| Occurrence {
                start_ms,
                end_ms: dur.map(|d| start_ms + d),
            })
            .filter(|o| o.start_ms >= now_ms || o.end_ms.is_some_and(|e| e > now_ms))
            .collect()
    }

    pub fn next_start(&self, clock: &ServerClock, now_ms: i64) -> Option<i64> {
        self.starts_between(clock, now_ms, now_ms + 8 * DAY_MS).first().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock() -> ServerClock {
        ServerClock {
            utc_offset_min: 540,
            reset_diario: TimeOfDay(960),
            reset_semanal_dia: 3,
        }
    }

    fn rule(json: &str) -> EventRule {
        serde_json::from_str(json).unwrap()
    }

    // 2026-10-14 (quarta) 00:00 UTC = 09:00 no servidor.
    const WED_00_UTC: i64 = 1_791_936_000_000;
    const H: i64 = 60 * MINUTE_MS;

    #[test]
    fn interval_every_3h_from_server_midnight() {
        let r = rule(r#"{"id":"f","nome":"Fenda","tipo":"intervalo","a_cada_min":180}"#);
        // Meia-noite do servidor (UTC+9) = 15:00 UTC; a cada 3 h cai em múltiplos de 3 h UTC.
        let s = r.starts_between(&clock(), WED_00_UTC, WED_00_UTC + 12 * H);
        assert_eq!(
            s,
            vec![WED_00_UTC, WED_00_UTC + 3 * H, WED_00_UTC + 6 * H, WED_00_UTC + 9 * H]
        );
    }

    #[test]
    fn interval_with_offset() {
        let r = rule(r#"{"id":"b","nome":"Beritra","tipo":"intervalo","a_cada_min":60,"offset_min":30}"#);
        assert_eq!(r.next_start(&clock(), WED_00_UTC), Some(WED_00_UTC + 30 * MINUTE_MS));
    }

    #[test]
    fn fixed_only_on_listed_weekdays() {
        // Domingo e sexta 21:00 servidor = 12:00 UTC do mesmo dia.
        let r = rule(r#"{"id":"n","nome":"Nahma","tipo":"fixo","horarios":["21:00"],"dias":[0,5]}"#);
        let fri_noon = WED_00_UTC + 2 * DAY_MS + 12 * H;
        assert_eq!(r.next_start(&clock(), WED_00_UTC), Some(fri_noon));
        assert_eq!(r.next_start(&clock(), fri_noon + 1), Some(fri_noon + 2 * DAY_MS));
    }

    #[test]
    fn ongoing_occurrence_is_listed() {
        let r = rule(r#"{"id":"x","nome":"X","tipo":"fixo","horarios":["09:00"],"duracao_min":30}"#);
        let occ = r.occurrences(&clock(), WED_00_UTC + 10 * MINUTE_MS, WED_00_UTC + H);
        assert_eq!(
            occ,
            vec![Occurrence {
                start_ms: WED_00_UTC,
                end_ms: Some(WED_00_UTC + 30 * MINUTE_MS)
            }]
        );
    }
}
