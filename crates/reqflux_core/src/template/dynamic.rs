//! Dynamic variables such as `{{$guid}}`.

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// A dynamic variable, resolved to a fresh value per occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicVariable {
    /// `$guid`: a UUID v4.
    Guid,
    /// `$randomUUID`: a UUID v4.
    RandomUuid,
    /// `$timestamp`: Unix time in seconds.
    Timestamp,
    /// `$isoTimestamp`: current time in RFC 3339, UTC.
    IsoTimestamp,
    /// `$randomInt`: an integer from 0 to 1000, inclusive.
    RandomInt,
}

impl DynamicVariable {
    /// Parses a reference name, including its `$`. Names are case-sensitive.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "$guid" => Some(Self::Guid),
            "$randomUUID" => Some(Self::RandomUuid),
            "$timestamp" => Some(Self::Timestamp),
            "$isoTimestamp" => Some(Self::IsoTimestamp),
            "$randomInt" => Some(Self::RandomInt),
            _ => None,
        }
    }
}

/// Produces the values of dynamic variables.
pub trait DynamicSource {
    /// Returns a fresh value for `variable`.
    fn generate(&mut self, variable: DynamicVariable) -> String;
}

/// The real source: random UUIDs, the system clock and a random number generator.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemDynamicSource;

impl DynamicSource for SystemDynamicSource {
    fn generate(&mut self, variable: DynamicVariable) -> String {
        match variable {
            DynamicVariable::Guid | DynamicVariable::RandomUuid => uuid::Uuid::new_v4().to_string(),
            DynamicVariable::Timestamp => OffsetDateTime::now_utc().unix_timestamp().to_string(),
            DynamicVariable::IsoTimestamp => iso_timestamp(OffsetDateTime::now_utc()),
            DynamicVariable::RandomInt => rand::random_range(0..=1000u32).to_string(),
        }
    }
}

/// Formats `time` as RFC 3339 in UTC with whole seconds, such as `2023-11-14T22:13:20Z`.
fn iso_timestamp(time: OffsetDateTime) -> String {
    let time = time.replace_nanosecond(0).unwrap_or(time);
    time.format(&Rfc3339)
        .unwrap_or_else(|_| time.unix_timestamp().to_string())
}

/// A deterministic source for tests.
///
/// Every call advances a counter `n`, starting at 1, shared by all variables: UUIDs are
/// `00000000-0000-4000-8000-{n:012}`, timestamps are `1_700_000_000 + n` and `$randomInt` is
/// `n % 1001`.
#[derive(Debug, Clone, Default)]
pub struct SequentialDynamicSource {
    calls: u64,
}

impl DynamicSource for SequentialDynamicSource {
    fn generate(&mut self, variable: DynamicVariable) -> String {
        self.calls += 1;
        let n = self.calls;
        let seconds = 1_700_000_000_i64.saturating_add_unsigned(n);
        match variable {
            DynamicVariable::Guid | DynamicVariable::RandomUuid => {
                format!("00000000-0000-4000-8000-{n:012}")
            }
            DynamicVariable::Timestamp => seconds.to_string(),
            DynamicVariable::IsoTimestamp => OffsetDateTime::from_unix_timestamp(seconds)
                .map_or_else(|_| seconds.to_string(), iso_timestamp),
            DynamicVariable::RandomInt => (n % 1001).to_string(),
        }
    }
}
