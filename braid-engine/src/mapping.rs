use crate::types::{Generator, Seat};

/// Safely maps a seat number to the valid range using modulo arithmetic.
/// 
/// Maps 1-based index to 0-based, applies modulo, then back to 1-based.
/// This prevents panics when seat numbers exceed total_seats due to player churn.
/// 
/// Examples:
/// - `safe_seat(10, 9)` returns `1` (10 wraps to 1)
/// - `safe_seat(12, 9)` returns `3` (12 wraps to 3)
/// - `safe_seat(5, 9)` returns `5` (already in range)
/// 
/// # Arguments
/// * `seat` - The seat number (1-based)
/// * `total` - Total number of seats (dimension)
/// 
/// # Returns
/// A seat number in the range [1, total]
pub(crate) fn safe_seat(seat: usize, total: usize) -> usize {
    if total == 0 {
        return 1; // Safety fallback
    }
    // Map 1-based to 0-based, modulo, then back to 1-based
    ((seat - 1) % total) + 1
}

/// Expands an action (movement from one seat to another) into a sequence of Artin generators.
/// 
/// Uses linear ordering 1..N for simplicity.
/// 
/// Logic:
/// - Movement i → i+1: emits σ_i
/// - Movement i → i-1: emits σ_{i-1}^{-1}
/// - Jumps: decomposed into adjacent swaps recursively
///   (e.g., 1 → 3 becomes 1 → 2 → 3, yielding σ_1, σ_2)
/// 
/// # Arguments
/// * `from` - The source seat (1-based)
/// * `to` - The target seat (1-based)
/// * `total_seats` - Total number of seats at the table
/// 
/// # Returns
/// A vector of generators representing the braid expansion
/// 
/// # Safety
/// Seat numbers are safely mapped using modulo arithmetic if they exceed total_seats.
/// This prevents panics when players churn and seat numbers grow beyond the dimension.
pub fn expand_action(from: Seat, to: Seat, total_seats: usize) -> Vec<Generator> {
    let from_val = from.value();
    let to_val = to.value();

    // Validate seat numbers are 1-based (must be > 0)
    if from_val == 0 || to_val == 0 {
        // Invalid seat, return empty (shouldn't happen with Seat::new, but be safe)
        return Vec::new();
    }

    // Apply safe mapping to handle seats that exceed total_seats
    let from_val = safe_seat(from_val, total_seats);
    let to_val = safe_seat(to_val, total_seats);

    // If same seat, no movement
    if from_val == to_val {
        return Vec::new();
    }

    let mut generators = Vec::new();
    let mut current = from_val;

    // Decompose the movement into adjacent swaps
    while current != to_val {
        if current < to_val {
            // Moving forward: emit σ_current
            generators.push(Generator::Sigma(current));
            current += 1;
        } else {
            // Moving backward: emit σ_{current-1}^{-1}
            generators.push(Generator::InverseSigma(current - 1));
            current -= 1;
        }
    }

    generators
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjacent_forward() {
        let from = Seat::new(1);
        let to = Seat::new(2);
        let result = expand_action(from, to, 4);
        assert_eq!(result, vec![Generator::Sigma(1)]);
    }

    #[test]
    fn test_adjacent_backward() {
        let from = Seat::new(2);
        let to = Seat::new(1);
        let result = expand_action(from, to, 4);
        assert_eq!(result, vec![Generator::InverseSigma(1)]);
    }

    #[test]
    fn test_jump_forward() {
        let from = Seat::new(1);
        let to = Seat::new(3);
        let result = expand_action(from, to, 4);
        assert_eq!(
            result,
            vec![Generator::Sigma(1), Generator::Sigma(2)]
        );
    }

    #[test]
    fn test_jump_backward() {
        let from = Seat::new(4);
        let to = Seat::new(2);
        let result = expand_action(from, to, 4);
        assert_eq!(
            result,
            vec![Generator::InverseSigma(3), Generator::InverseSigma(2)]
        );
    }

    #[test]
    fn test_same_seat() {
        let seat = Seat::new(2);
        let result = expand_action(seat, seat, 4);
        assert_eq!(result, Vec::<Generator>::new());
    }

    #[test]
    fn test_seat_modulo_wrapping() {
        // Test that seats exceeding total_seats are safely wrapped
        // Seat 10 with dimension 9 should wrap to 1
        let from = Seat::new(10);
        let to = Seat::new(11);
        let result = expand_action(from, to, 9);
        // 10 wraps to 1, 11 wraps to 2, so 1->2 = σ₁
        assert_eq!(result, vec![Generator::Sigma(1)]);
    }

    #[test]
    fn test_seat_modulo_large_numbers() {
        // Test with very large seat numbers
        let from = Seat::new(25);
        let to = Seat::new(26);
        let result = expand_action(from, to, 9);
        // 25 % 9 = 7, 26 % 9 = 8, so 7->8 = σ₇
        assert_eq!(result, vec![Generator::Sigma(7)]);
    }

    #[test]
    fn test_safe_seat_function() {
        // Direct test of safe_seat helper
        assert_eq!(safe_seat(10, 9), 1);  // 10 wraps to 1
        assert_eq!(safe_seat(12, 9), 3);  // 12 wraps to 3
        assert_eq!(safe_seat(5, 9), 5);   // Already in range
        assert_eq!(safe_seat(18, 9), 9);  // 18 wraps to 9
        assert_eq!(safe_seat(1, 9), 1);   // Edge case: minimum
        assert_eq!(safe_seat(9, 9), 9);   // Edge case: maximum
    }
}

