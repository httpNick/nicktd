use serde::{Deserialize, Serialize};

/// A player's chosen tower family for the match. Only `Basic` (today's three
/// shapes) exists in this chunk; Ice/Poison Beasts/Elves land in Chunks 3-5.
/// Deliberately just an enum, not a struct carrying data: the catalog lookup
/// (`unit_config::family_catalog`) is the single source of truth for which
/// `UnitKind`s a family unlocks, so adding a family later is one match arm,
/// not a schema change.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Basic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_serializes_as_plain_string() {
        assert_eq!(serde_json::to_string(&Family::Basic).unwrap(), "\"Basic\"");
    }
}
