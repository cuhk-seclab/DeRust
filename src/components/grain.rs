use std::fmt::{Display, Formatter};

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum RudraGrain {
    Low = 0,
    Medium = 1,
    High = 2,
    Ultra = 3,
}

impl Display for RudraGrain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                RudraGrain::Ultra => "Ultra",
                RudraGrain::High => "High",
                RudraGrain::Medium => "Medium",
                RudraGrain::Low => "Low,",
            }
        )
    }
}
