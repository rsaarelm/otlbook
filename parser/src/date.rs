use chrono::{naive::NaiveDate, offset::FixedOffset, DateTime, Datelike};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// Date values that may be expressed with different levels of detail.
///
/// The `VagueDate` value will remember the level of detail used.
///
/// NB: The type is used in the outline data, so the string representations are all expected to
/// have no whitespace.
///
/// Default representations:
///
/// * Year: `"2006"`
/// * YearMonth: `"2006-01"`
/// * Date: `"2006-01-02"`
/// * DateTime: `"2006-01-02T15:04:05-0700"`
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum VagueDate {
    Year(i32),
    YearMonth(i32, u32),
    Date(NaiveDate),
    DateTime(DateTime<FixedOffset>),
}

use VagueDate::*;

impl VagueDate {
    /// Reduce precision to the level of the other date.
    ///
    /// Ie if the other date is YearMonth, 2006-01-02 becomes 2006-01.
    fn reduce_precision_to(&self, other: &VagueDate) -> VagueDate {
        // Hack: Use the string representation and the fixed lenghts of the less precise types to
        // do this.
        match other {
            DateTime(_) => *self,
            Date(_) => (&format!("{}", self)[..10]).parse().unwrap(),
            YearMonth(_, _) => (&format!("{}", self)[..7]).parse().unwrap(),
            Year(_) => (&format!("{}", self)[..4]).parse().unwrap(),
        }
    }

    /// Value is arbitrary, but more precision is bigger.
    fn precision(&self) -> usize {
        match self {
            Year(_) => 1,
            YearMonth(_, _) => 2,
            Date(_) => 3,
            DateTime(_) => 4,
        }
    }
}

impl Ord for VagueDate {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_reduced = self.reduce_precision_to(other);
        let other_reduced = other.reduce_precision_to(self);

        let reduced_cmp = match (self_reduced, other_reduced) {
            (Year(a), Year(b)) => a.cmp(&b),
            (YearMonth(ay, am), YearMonth(by, bm)) => (ay, am).cmp(&(by, bm)),
            (Date(a), Date(b)) => a.cmp(&b),
            (DateTime(a), DateTime(b)) => a.cmp(&b),
            _ => panic!("reduce_precision_to failed"),
        };

        if reduced_cmp == Ordering::Equal {
            self.precision().cmp(&other.precision())
        } else {
            reduced_cmp
        }
    }
}

impl PartialOrd for VagueDate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// May need a NaiveDateTime without timezone information here later. Currently assuming that the
// fuzzy dates
//
// No plan to handle BCE years sensibly if those are ever needed.

impl FromStr for VagueDate {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z") {
            Ok(DateTime(dt))
        } else if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m-%d") {
            Ok(Date(dt.date().naive_local()))
        } else if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m") {
            Ok(YearMonth(dt.year(), dt.month()))
        } else if let Ok(dt) = DateTime::parse_from_str(s, "%Y") {
            Ok(Year(dt.year()))
        } else {
            Err(())
        }
    }
}

impl fmt::Display for VagueDate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Year(y) => write!(f, "{}", y),
            YearMonth(y, m) => write!(f, "{}-{}", y, m),
            Date(date) => write!(f, "{}", date.format("%Y-%m-%d")),
            DateTime(date_time) => write!(f, "{}", date_time.format("%Y-%m-%dT%H:%M:%S%z")),
        }
    }
}
