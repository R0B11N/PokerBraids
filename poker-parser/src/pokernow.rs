use braid_engine::ActionType;
use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;

/// PokerNow CSV row structure.
/// 
/// PokerNow logs have columns: "entry", "at", "order"
#[derive(Debug, Deserialize)]
pub struct PokerNowRow {
    /// The log entry text (e.g., "Alice @ p1 raises to 200")
    #[serde(rename = "entry")]
    pub entry: String,
    /// Timestamp (we parse but don't use for braid calculation)
    #[serde(rename = "at")]
    pub at: String,
    /// Order number
    #[serde(rename = "order")]
    pub order: u64,
}

// Master regex for parsing PokerNow log entries.
// Pattern supports:
// - Optional timestamp: "23:18 "
// - Hand reset delimiter: "-- starting hand"
// - Optional player ID: "@ p1" (can be missing in live DOM)
// - Action keywords: folds, checks, calls, bets, raises, posts, etc.
// - Optional amount: "90" or "90.5"
// Matches lines like:
// - "-- starting hand #5 --" (hand delimiter)
// - "Alice @ p1 folds" (CSV format)
// - "23:18 le_chiffre calls 90" (Live DOM format)
// - "Bob @ p2 calls 50" (CSV format)
// - "Charlie raises to 200" (Live DOM format without ID)
lazy_static! {
    static ref POKERNOW_REGEX: Regex = Regex::new(
        r"^(?:(?P<time>\d{1,2}:\d{2})\s+)?(?:(?P<reset>-- starting hand)|(?P<name>.+?)(?: @ (?P<id>.+?))? (?P<action>folds|checks|calls|bets|raises|shows|quits|joins|posts))(?: to | )?(?P<amount>[\d\.]+)?"
    ).expect("Invalid PokerNow regex pattern");
}

/// Parses a PokerNow row and extracts action information.
/// 
/// # Arguments
/// * `row` - The PokerNowRow to parse
/// 
/// # Returns
/// `Some((player_id, action_type, amount))` if the row contains a valid action,
/// `None` if the row should be filtered out (e.g., system messages, chat, etc.)
/// 
/// # Player ID Generation
/// Combines name and ID (e.g., "Alice_p1") to ensure uniqueness if people share names.
/// For Reset actions, player_id is "system_reset".
pub fn parse_row(row: &PokerNowRow) -> Option<(String, ActionType, u64)> {
    // Try to match the regex
    let caps = POKERNOW_REGEX.captures(&row.entry)?;
    
    // Check for hand reset delimiter first
    if caps.name("reset").is_some() {
        // This is a "starting hand" line
        return Some(("system_reset".to_string(), ActionType::Reset, 0));
    }
    
    // Extract name (required for non-reset actions)
    let name = caps.name("name")?.as_str().trim();
    
    // Extract ID (optional - may be missing in live DOM format)
    let id = caps.name("id").map(|m| m.as_str().trim());
    
    // Generate unique player ID
    // If ID exists: "name_id", otherwise: "name_generated"
    let player_id = if let Some(id_str) = id {
        if !id_str.is_empty() {
            format!("{}_{}", name, id_str)
        } else {
            format!("{}_generated", name)
        }
    } else {
        format!("{}_generated", name)
    };
    
    let action_str = caps.name("action")?.as_str().to_lowercase();
    
    // Parse action type
    let action_type = match action_str.as_str() {
        "folds" => ActionType::Fold,
        "checks" => ActionType::Check,
        "calls" => ActionType::Call,
        "bets" => ActionType::Bet,
        "raises" => ActionType::Raise,
        "posts" => ActionType::Bet, // Map blinds/posts to Bet
        "shows" | "quits" | "joins" => {
            // Filter out non-betting actions
            return None;
        }
        _ => {
            // Unknown action type, filter out
            return None;
        }
    };
    
    // Parse amount (handles both integer and decimal formats)
    let amount = match caps.name("amount") {
        Some(amt) => {
            let amt_str = amt.as_str();
            // Try parsing as f64 first (handles decimals), then convert to u64
            amt_str
                .parse::<f64>()
                .map(|f| f as u64)
                .unwrap_or_else(|_| {
                    // Fallback to integer parsing
                    amt_str.parse::<u64>().unwrap_or(0)
                })
        }
        None => 0,
    };
    
    // For actions that don't have amounts (fold, check), amount is 0
    let final_amount = match action_type {
        ActionType::Fold | ActionType::Check => 0,
        _ => amount,
    };
    
    Some((player_id, action_type, final_amount))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fold() {
        let row = PokerNowRow {
            entry: "Alice @ p1 folds".to_string(),
            at: "2025-01-01T12:00:00".to_string(),
            order: 1,
        };
        
        let result = parse_row(&row);
        assert!(result.is_some());
        let (player_id, action_type, amount) = result.unwrap();
        assert_eq!(player_id, "Alice_p1");
        assert_eq!(action_type, ActionType::Fold);
        assert_eq!(amount, 0);
    }

    #[test]
    fn test_parse_check() {
        let row = PokerNowRow {
            entry: "Bob @ p2 checks".to_string(),
            at: "2025-01-01T12:00:01".to_string(),
            order: 2,
        };
        
        let result = parse_row(&row);
        assert!(result.is_some());
        let (player_id, action_type, amount) = result.unwrap();
        assert_eq!(player_id, "Bob_p2");
        assert_eq!(action_type, ActionType::Check);
        assert_eq!(amount, 0);
    }

    #[test]
    fn test_parse_call() {
        let row = PokerNowRow {
            entry: "Charlie @ p3 calls 50".to_string(),
            at: "2025-01-01T12:00:02".to_string(),
            order: 3,
        };
        
        let result = parse_row(&row);
        assert!(result.is_some());
        let (player_id, action_type, amount) = result.unwrap();
        assert_eq!(player_id, "Charlie_p3");
        assert_eq!(action_type, ActionType::Call);
        assert_eq!(amount, 50);
    }

    #[test]
    fn test_parse_bet() {
        let row = PokerNowRow {
            entry: "Dave @ p4 bets 100".to_string(),
            at: "2025-01-01T12:00:03".to_string(),
            order: 4,
        };
        
        let result = parse_row(&row);
        assert!(result.is_some());
        let (player_id, action_type, amount) = result.unwrap();
        assert_eq!(player_id, "Dave_p4");
        assert_eq!(action_type, ActionType::Bet);
        assert_eq!(amount, 100);
    }

    #[test]
    fn test_parse_raises_to() {
        let row = PokerNowRow {
            entry: "Alice @ p1 raises to 200".to_string(),
            at: "2025-01-01T12:00:04".to_string(),
            order: 5,
        };
        
        let result = parse_row(&row);
        assert!(result.is_some());
        let (player_id, action_type, amount) = result.unwrap();
        assert_eq!(player_id, "Alice_p1");
        assert_eq!(action_type, ActionType::Raise);
        assert_eq!(amount, 200);
    }

    #[test]
    fn test_parse_shows_filtered() {
        let row = PokerNowRow {
            entry: "Alice @ p1 shows hand ...".to_string(),
            at: "2025-01-01T12:00:05".to_string(),
            order: 6,
        };
        
        let result = parse_row(&row);
        assert!(result.is_none(), "Shows action should be filtered out");
    }

    #[test]
    fn test_parse_system_message_filtered() {
        let row = PokerNowRow {
            entry: "System: Player xyz joined".to_string(),
            at: "2025-01-01T12:00:06".to_string(),
            order: 7,
        };
        
        let result = parse_row(&row);
        assert!(result.is_none(), "System messages should be filtered out");
    }

    #[test]
    fn test_player_id_uniqueness() {
        // Test that same name with different IDs gets different player_ids
        let row1 = PokerNowRow {
            entry: "Alice @ p1 calls 50".to_string(),
            at: "2025-01-01T12:00:00".to_string(),
            order: 1,
        };
        
        let row2 = PokerNowRow {
            entry: "Alice @ p2 calls 50".to_string(),
            at: "2025-01-01T12:00:01".to_string(),
            order: 2,
        };
        
        let result1 = parse_row(&row1);
        let result2 = parse_row(&row2);
        
        assert!(result1.is_some());
        assert!(result2.is_some());
        
        assert_eq!(result1.unwrap().0, "Alice_p1");
        assert_eq!(result2.unwrap().0, "Alice_p2");
    }

    #[test]
    fn test_parse_live_dom_format_with_timestamp() {
        // Test live DOM format: "23:18 le_chiffre calls 90"
        let row = PokerNowRow {
            entry: "23:18 le_chiffre calls 90".to_string(),
            at: "2025-01-01T12:00:00".to_string(),
            order: 1,
        };
        
        let result = parse_row(&row);
        assert!(result.is_some(), "Should parse live DOM format with timestamp");
        let (player_id, action_type, amount) = result.unwrap();
        assert_eq!(player_id, "le_chiffre_generated", "Should generate ID when missing");
        assert_eq!(action_type, ActionType::Call);
        assert_eq!(amount, 90);
    }

    #[test]
    fn test_parse_live_dom_format_without_timestamp() {
        // Test live DOM format without timestamp: "le_chiffre calls 90"
        let row = PokerNowRow {
            entry: "le_chiffre calls 90".to_string(),
            at: "2025-01-01T12:00:00".to_string(),
            order: 1,
        };
        
        let result = parse_row(&row);
        assert!(result.is_some(), "Should parse live DOM format without timestamp");
        let (player_id, action_type, amount) = result.unwrap();
        assert_eq!(player_id, "le_chiffre_generated");
        assert_eq!(action_type, ActionType::Call);
        assert_eq!(amount, 90);
    }

    #[test]
    fn test_parse_mixed_formats() {
        // Test that both formats work
        let csv_row = PokerNowRow {
            entry: "Alice @ p1 raises to 200".to_string(),
            at: "2025-01-01T12:00:00".to_string(),
            order: 1,
        };
        
        let live_row = PokerNowRow {
            entry: "23:18 Bob bets 100".to_string(),
            at: "2025-01-01T12:00:01".to_string(),
            order: 2,
        };
        
        let csv_result = parse_row(&csv_row);
        let live_result = parse_row(&live_row);
        
        assert!(csv_result.is_some());
        assert!(live_result.is_some());
        
        assert_eq!(csv_result.unwrap().0, "Alice_p1");
        assert_eq!(live_result.unwrap().0, "Bob_generated");
    }

    #[test]
    fn test_parse_hand_reset() {
        // Test hand reset delimiter detection
        let row = PokerNowRow {
            entry: "-- starting hand #5 --".to_string(),
            at: "2025-01-01T12:00:00".to_string(),
            order: 1,
        };
        
        let result = parse_row(&row);
        assert!(result.is_some(), "Should parse hand reset delimiter");
        let (player_id, action_type, amount) = result.unwrap();
        assert_eq!(player_id, "system_reset");
        assert_eq!(action_type, ActionType::Reset);
        assert_eq!(amount, 0);
    }

    #[test]
    fn test_parse_hand_reset_with_timestamp() {
        // Test hand reset with timestamp
        let row = PokerNowRow {
            entry: "23:18 -- starting hand #3 --".to_string(),
            at: "2025-01-01T12:00:00".to_string(),
            order: 1,
        };
        
        let result = parse_row(&row);
        assert!(result.is_some(), "Should parse hand reset with timestamp");
        let (player_id, action_type, _) = result.unwrap();
        assert_eq!(player_id, "system_reset");
        assert_eq!(action_type, ActionType::Reset);
    }

    #[test]
    fn test_parse_posts_action() {
        // Test that "posts" (blinds) maps to Bet
        let row = PokerNowRow {
            entry: "Alice @ p1 posts 10".to_string(),
            at: "2025-01-01T12:00:00".to_string(),
            order: 1,
        };
        
        let result = parse_row(&row);
        assert!(result.is_some(), "Should parse posts action");
        let (_, action_type, amount) = result.unwrap();
        assert_eq!(action_type, ActionType::Bet);
        assert_eq!(amount, 10);
    }
}

