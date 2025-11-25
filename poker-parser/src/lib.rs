use braid_engine::{Action, ActionType, Seat};
use csv::StringRecord;
use std::collections::HashMap;
use std::error::Error;

pub mod pokernow;

/// Parses a CSV record into an Action.
/// 
/// Expected CSV format: player_id,action,amount
/// 
/// # Arguments
/// * `record` - A CSV string record
/// * `seat_resolver` - Resolver to map player_id to Seat
/// 
/// # Returns
/// A Result containing the parsed Action or an error

pub fn parse_record(
    record: &StringRecord,
    seat_resolver: &mut SeatResolver,
) -> Result<Action, Box<dyn Error>> {
    if record.len() < 3 {
        return Err("CSV record must have at least 3 fields: player_id, action, amount".into());
    }

    let player_id = record.get(0).ok_or("Missing player_id field")?;
    let action_str = record.get(1).ok_or("Missing action field")?;
    let amount_str = record.get(2).ok_or("Missing amount field")?;

    // Resolve player_id to Seat
    let seat = seat_resolver.get_or_assign_seat(player_id);

    // Parse action string to ActionType
    let action_type = parse_action_type(action_str)?;

    // Parse amount
    let amount = amount_str
        .parse::<u64>()
        .map_err(|e| format!("Invalid amount '{}': {}", amount_str, e))?;

    Ok(Action::new(seat, action_type, amount))
}

/// Parses an action string into an ActionType enum.
/// 
/// Supported actions:
/// - "bet" -> ActionType::Bet
/// - "raise" -> ActionType::Raise
/// - "check" -> ActionType::Check
/// - "call" -> ActionType::Call
/// - "fold" -> ActionType::Fold
/// - "reraise" or "re-raise" -> ActionType::ReRaise
/// - "allin" or "all-in" -> ActionType::AllIn
fn parse_action_type(action_str: &str) -> Result<ActionType, Box<dyn Error>> {
    match action_str.to_lowercase().trim() {
        "bet" => Ok(ActionType::Bet),
        "raise" => Ok(ActionType::Raise),
        "check" => Ok(ActionType::Check),
        "call" => Ok(ActionType::Call),
        "fold" => Ok(ActionType::Fold),
        "reraise" | "re-raise" => Ok(ActionType::ReRaise),
        "allin" | "all-in" => Ok(ActionType::AllIn),
        _ => Err(format!("Unknown action type: '{}'", action_str).into()),
    }
}

/// Resolves player IDs to Seat numbers, thereby assigning seats sequentially as new player IDs appear in the stream.

#[derive(Debug, Clone)]
pub struct SeatResolver {
    player_to_seat: HashMap<String, Seat>,
    next_seat: usize,
}

impl SeatResolver {
    /// Creates a new SeatResolver.
    pub fn new() -> Self {
        SeatResolver {
            player_to_seat: HashMap::new(),
            next_seat: 1, // Start with seat 1 (1-based indexing)
        }
    }

    /// Gets the Seat for a player ID, or assigns a new seat if the player is new.
    /// 
    /// # Arguments
    /// * `player_id` - The player identifier (name, ID, etc.)
    /// 
    /// # Returns
    /// The Seat assigned to this player
    /// 
    /// # Name Update Logic
    /// If the player_id contains an ID part (after `_`), we try to match
    /// existing seats by ID and update the name. This allows `[S#]` tags to propagate.
    pub fn get_or_assign_seat(&mut self, player_id: &str) -> Seat {
        let player_id = player_id.trim().to_string();
        
        // Try exact match first
        if let Some(&seat) = self.player_to_seat.get(&player_id) {
            return seat;
        }
        
        // Try to match by ID part (for name updates like "PlayerName_ID" -> "[S5] PlayerName_ID")
        // Extract ID part: look for pattern "name_ID" or "name_generated"
        if let Some(id_part) = player_id.split('_').last() {
            // Search for existing entries with the same ID part
            // Collect matching entries first to avoid borrowing issues
            let mut matching_entry: Option<(String, Seat)> = None;
            for (existing_id, &existing_seat) in &self.player_to_seat {
                if existing_id.ends_with(&format!("_{}", id_part)) && existing_id != &player_id {
                    matching_entry = Some((existing_id.clone(), existing_seat));
                    break;
                }
            }
            
            if let Some((old_id, seat)) = matching_entry {
                // Found existing seat with same ID - update the mapping with new name
                self.player_to_seat.remove(&old_id);
                self.player_to_seat.insert(player_id, seat);
                return seat;
            }
        }
        
        // New player - assign new seat
        let seat = Seat::new(self.next_seat);
        self.player_to_seat.insert(player_id, seat);
        self.next_seat += 1;
        seat
    }

    /// Returns the total number of unique players seen so far.
    pub fn player_count(&self) -> usize {
        self.player_to_seat.len()
    }

    /// Returns the maximum seat number assigned.
    pub fn max_seat(&self) -> usize {
        self.next_seat - 1
    }

    /// Gets the player ID (name) for a given seat.
    /// 
    /// # Arguments
    /// * `seat` - The seat to look up
    /// 
    /// # Returns
    /// The player ID string, or "Unknown" if not found
    pub fn get_player_name(&self, seat: Seat) -> String {
        self.player_to_seat
            .iter()
            .find(|(_, &s)| s == seat)
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| format!("Seat {}", seat.value()))
    }
}

impl Default for SeatResolver {
    fn default() -> Self {
        SeatResolver::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action_type() {
        assert_eq!(parse_action_type("bet").unwrap(), ActionType::Bet);
        assert_eq!(parse_action_type("raise").unwrap(), ActionType::Raise);
        assert_eq!(parse_action_type("check").unwrap(), ActionType::Check);
        assert_eq!(parse_action_type("call").unwrap(), ActionType::Call);
        assert_eq!(parse_action_type("fold").unwrap(), ActionType::Fold);
        assert_eq!(parse_action_type("reraise").unwrap(), ActionType::ReRaise);
        assert_eq!(parse_action_type("allin").unwrap(), ActionType::AllIn);
    }

    #[test]
    fn test_parse_action_type_case_insensitive() {
        assert_eq!(parse_action_type("BET").unwrap(), ActionType::Bet);
        assert_eq!(parse_action_type("Raise").unwrap(), ActionType::Raise);
        assert_eq!(parse_action_type("  call  ").unwrap(), ActionType::Call);
    }

    #[test]
    fn test_seat_resolver() {
        let mut resolver = SeatResolver::new();
        
        let seat1 = resolver.get_or_assign_seat("Alice");
        assert_eq!(seat1.value(), 1);
        
        let seat2 = resolver.get_or_assign_seat("Bob");
        assert_eq!(seat2.value(), 2);
        
        // Alice should get the same seat
        let seat1_again = resolver.get_or_assign_seat("Alice");
        assert_eq!(seat1_again.value(), 1);
        
        assert_eq!(resolver.player_count(), 2);
        assert_eq!(resolver.max_seat(), 2);
    }

    #[test]
    fn test_parse_record() {
        let mut resolver = SeatResolver::new();
        let mut record = StringRecord::new();
        record.push_field("Alice");
        record.push_field("raise");
        record.push_field("100");
        
        let action = parse_record(&record, &mut resolver).unwrap();
        assert_eq!(action.seat.value(), 1);
        assert_eq!(action.action_type, ActionType::Raise);
        assert_eq!(action.amount, 100);
    }
}
