//! Relógio do servidor: resets diário e semanal no fuso do servidor, não do PC.

use serde::{Deserialize, Serialize};

pub const MINUTE_MS: i64 = 60_000;
pub const DAY_MS: i64 = 24 * 60 * MINUTE_MS;
pub const WEEK_MS: i64 = 7 * DAY_MS;

/// Hora do dia em minutos desde a meia-noite, lida de `"HH:MM"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct TimeOfDay(pub u32);

impl TryFrom<String> for TimeOfDay {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        let (h, m) = s.split_once(':').ok_or_else(|| format!("hora inválida: {s}"))?;
        let h: u32 = h.trim().parse().map_err(|_| format!("hora inválida: {s}"))?;
        let m: u32 = m.trim().parse().map_err(|_| format!("hora inválida: {s}"))?;
        if h > 23 || m > 59 {
            return Err(format!("hora inválida: {s}"));
        }
        Ok(TimeOfDay(h * 60 + m))
    }
}

impl From<TimeOfDay> for String {
    fn from(t: TimeOfDay) -> String {
        format!("{:02}:{:02}", t.0 / 60, t.0 % 60)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    Diario,
    Semanal,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerClock {
    /// Fuso do relógio do servidor em minutos (UTC+9 = 540).
    pub utc_offset_min: i32,
    pub reset_diario: TimeOfDay,
    /// 0 = domingo ... 6 = sábado.
    pub reset_semanal_dia: u8,
}

impl ServerClock {
    fn offset_ms(&self) -> i64 {
        self.utc_offset_min as i64 * MINUTE_MS
    }

    /// Converte UTC para "milissegundos no relógio do servidor".
    pub fn to_server(&self, utc_ms: i64) -> i64 {
        utc_ms + self.offset_ms()
    }

    pub fn to_utc(&self, server_ms: i64) -> i64 {
        server_ms - self.offset_ms()
    }

    /// Dia da semana (0 = domingo) de um instante no relógio do servidor.
    pub fn weekday(server_ms: i64) -> u8 {
        // 1970-01-01 foi uma quinta-feira.
        (server_ms.div_euclid(DAY_MS) + 4).rem_euclid(7) as u8
    }

    /// Início do período atual (último reset), em UTC.
    pub fn period_start(&self, period: Period, now_ms: i64) -> i64 {
        let local = self.to_server(now_ms);
        let mut start = local.div_euclid(DAY_MS) * DAY_MS + self.reset_diario.0 as i64 * MINUTE_MS;
        if start > local {
            start -= DAY_MS;
        }
        if period == Period::Semanal {
            let back = (Self::weekday(start) as i64 - self.reset_semanal_dia as i64).rem_euclid(7);
            start -= back * DAY_MS;
        }
        self.to_utc(start)
    }

    /// Próximo reset, em UTC.
    pub fn next_reset(&self, period: Period, now_ms: i64) -> i64 {
        self.period_start(period, now_ms)
            + match period {
                Period::Diario => DAY_MS,
                Period::Semanal => WEEK_MS,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn global() -> ServerClock {
        ServerClock {
            utc_offset_min: 540,
            reset_diario: TimeOfDay(16 * 60),
            reset_semanal_dia: 3,
        }
    }

    // 2026-10-14 é uma quarta-feira.
    const WED_0659_UTC: i64 = 1_791_961_140_000; // 2026-10-14 06:59 UTC
    const WED_0700_UTC: i64 = 1_791_961_200_000; // 2026-10-14 07:00 UTC

    #[test]
    fn time_of_day_parse() {
        assert_eq!(TimeOfDay::try_from("16:00".to_string()).unwrap(), TimeOfDay(960));
        assert!(TimeOfDay::try_from("24:00".to_string()).is_err());
        assert!(TimeOfDay::try_from("x".to_string()).is_err());
        assert_eq!(String::from(TimeOfDay(65)), "01:05");
    }

    #[test]
    fn weekday_of_known_date() {
        assert_eq!(ServerClock::weekday(0), 4);
        assert_eq!(ServerClock::weekday(WED_0700_UTC), 3);
    }

    #[test]
    fn daily_reset_at_16_server_is_07_utc() {
        let c = global();
        assert_eq!(c.period_start(Period::Diario, WED_0700_UTC), WED_0700_UTC);
        assert_eq!(c.period_start(Period::Diario, WED_0659_UTC), WED_0700_UTC - DAY_MS);
        assert_eq!(c.next_reset(Period::Diario, WED_0659_UTC), WED_0700_UTC);
    }

    #[test]
    fn weekly_reset_on_wednesday() {
        let c = global();
        assert_eq!(c.period_start(Period::Semanal, WED_0700_UTC), WED_0700_UTC);
        // Um minuto antes, o período semanal ainda é o da quarta anterior.
        assert_eq!(c.period_start(Period::Semanal, WED_0659_UTC), WED_0700_UTC - WEEK_MS);
        // Sábado seguinte continua no período que começou na quarta.
        assert_eq!(c.period_start(Period::Semanal, WED_0700_UTC + 3 * DAY_MS), WED_0700_UTC);
        assert_eq!(
            c.next_reset(Period::Semanal, WED_0700_UTC + DAY_MS),
            WED_0700_UTC + WEEK_MS
        );
    }
}
