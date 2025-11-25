use crate::types::Generator;
use nalgebra::DMatrix;
use num_complex::Complex;
use std::collections::HashMap;
use serde::Serialize;

/// Player-specific metrics for topological profiling.
#[derive(Debug, Clone, Serialize, Default)]
pub struct PlayerMetrics {
    pub name: String,     // e.g. "Alex202"
    pub writhe: i32,      // Net cumulative crossings initiated by this player
    pub complexity: f64,  // Personal entanglement (Diagonal of Burau Matrix)
}

/// Trait for incremental updates to fingerprint state.
/// This allows streaming updates as generators are processed.
pub trait IncrementalUpdate {
    /// Updates the state with a new generator.
    fn update(&mut self, gen: &Generator);
}

/// Fingerprint state for braid invariants.
/// Implements a tiered strategy:
/// - Tier 1: Instant (integer arithmetic only)
/// - Tier 2: Fast (linear algebra / Burau representation)
/// - Tier 3: Slow (Jones polynomial, computed on demand)
#[derive(Debug, Clone)]
pub struct FingerprintState {
    // Tier 1: Instant (Integer arithmetic only)
    pub writhe: i32,
    pub crossing_count: usize,

    // Tier 2: Fast (Linear Algebra / Burau Representation)
    /// Burau matrix representation (N x N, where N is the number of strands/seats)
    pub burau_matrix: DMatrix<Complex<f64>>,
    /// Complex parameter t for Burau representation (e^(i * 1.0) - "Golden Phase")
    pub t_param: Complex<f64>,
    /// Dimension of the braid (number of seats)
    dimension: usize,

    // Tier 3: Slow (Jones Polynomial)
    // Only computed on demand, not incrementally updated
    pub jones_poly_cache: Option<String>,

    // Player-Specific Profiling
    /// Per-seat metrics for individual player tracking
    pub player_stats: HashMap<usize, PlayerMetrics>,
}

impl FingerprintState {
    /// Creates a new empty fingerprint state with Burau matrix initialized to identity.
    /// 
    /// # Arguments
    /// * `dimension` - Number of strands/seats (typically 9 for max poker table)
    pub fn new(dimension: usize) -> Self {
        // Golden Phase: t = e^(i * 1.0) = cos(1.0) + i*sin(1.0)
        let t_param = Complex::new(1.0_f64.cos(), 1.0_f64.sin());
        
        // Initialize Burau matrix as identity
        let burau_matrix = DMatrix::identity(dimension, dimension);

        FingerprintState {
            writhe: 0,
            crossing_count: 0,
            burau_matrix,
            t_param,
            dimension,
            jones_poly_cache: None,
            player_stats: HashMap::new(),
        }
    }

    /// Creates a new fingerprint state with default dimension (9 seats).
    pub fn with_default_dimension() -> Self {
        Self::new(9)
    }

    /// Resets the state to initial values.
    /// Resets the Burau matrix to identity and clears player stats.
    pub fn reset(&mut self) {
        self.writhe = 0;
        self.crossing_count = 0;
        self.burau_matrix = DMatrix::identity(self.dimension, self.dimension);
        self.player_stats.clear();
    }

    /// Returns the dimension of the braid.
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Calculates the Burau trace magnitude.
    /// 
    /// This is the magnitude of the trace (sum of diagonal elements) of the Burau matrix.
    /// This scalar invariant is suitable for display on a HUD and represents the
    /// "energy" or "complexity" of the hand.
    /// 
    /// # Returns
    /// The magnitude (norm) of the complex trace
    pub fn burau_trace_magnitude(&self) -> f64 {
        let trace = self.burau_matrix.diagonal().iter().sum::<Complex<f64>>();
        trace.norm()
    }

    /// Updates the fingerprint state with a generator and tracks per-seat metrics.
    /// 
    /// This method updates both global and per-seat statistics when a generator
    /// is applied due to an action by a specific player.
    /// 
    /// # Arguments
    /// * `gen` - The generator to apply
    /// * `seat` - The seat (1-based) that initiated this action
    /// * `name` - The player name for this seat
    pub fn update_for_seat(&mut self, gen: &Generator, seat: usize, name: String) {
        // Update global state
        self.update(gen);

        // Ensure seat is in valid range (1-based)
        if seat == 0 || seat > self.dimension {
            return;
        }

        // Get or create player metrics
        let metrics = self.player_stats.entry(seat).or_insert_with(|| {
            PlayerMetrics {
                name: name.clone(),
                writhe: 0,
                complexity: 0.0,
            }
        });

        // ALWAYS update the name to catch tag updates from the bridge
        // This ensures [S#] tags propagate to the visualizer when they appear
        if !name.is_empty() {
            metrics.name = name;
        }

        // Update writhe for this seat
        match gen {
            Generator::Sigma(_) => {
                metrics.writhe += 1;
            }
            Generator::InverseSigma(_) => {
                metrics.writhe -= 1;
            }
        }

        // Update complexity: extract diagonal element from Burau matrix
        // Seat is 1-based, so index is seat - 1
        let seat_index = seat - 1;
        if seat_index < self.dimension {
            let diagonal_element = self.burau_matrix[(seat_index, seat_index)];
            metrics.complexity = diagonal_element.norm();
        }
    }

    /// Processes an action and updates the fingerprint state.
    /// 
    /// If the action is a Reset, the state is reset to identity.
    /// Otherwise, the action is expanded to generators and applied incrementally.
    /// 
    /// # Arguments
    /// * `action` - The action to process
    /// * `current_seat` - Current seat (for action expansion)
    /// 
    /// # Returns
    /// The number of generators applied (0 for Reset)
    pub fn process_action(
        &mut self,
        action: &crate::types::Action,
        current_seat: Option<crate::types::Seat>,
    ) -> usize {
        use crate::types::ActionType;
        
        if action.action_type == ActionType::Reset {
            self.reset();
            return 0;
        }
        
        // Expand action to generators
        let from_seat = current_seat.unwrap_or(action.seat);
        let generators = crate::mapping::expand_action(from_seat, action.seat, self.dimension());
        
        // Apply each generator
        for gen in &generators {
            self.update(gen);
        }
        
        generators.len()
    }
}

impl Default for FingerprintState {
    fn default() -> Self {
        FingerprintState::with_default_dimension()
    }
}

impl IncrementalUpdate for FingerprintState {
    /// Updates the fingerprint state with a new generator.
    /// 
    /// Updates:
    /// - writhe: +1 for Sigma (overcrossing), -1 for InverseSigma (undercrossing)
    /// - crossing_count: incremented by 1
    /// - Burau matrix: multiplied by generator matrix U_k or U_k^{-1}
    fn update(&mut self, gen: &Generator) {
        match gen {
            Generator::Sigma(k) => {
                self.writhe += 1;
                self.apply_sigma_matrix(*k);
            }
            Generator::InverseSigma(k) => {
                self.writhe -= 1;
                self.apply_inverse_sigma_matrix(*k);
            }
        }
        self.crossing_count += 1;
    }
}

impl FingerprintState {
    /// Applies the generator matrix U_k for σ_k to the Burau matrix.
    /// 
    /// U_k is the identity matrix except for the 2x2 block at indices (k-1, k):
    /// [1-t  t ]
    /// [1    0 ]
    /// 
    /// Note: k is 1-based, so we use indices k-1 and k (0-based).
    fn apply_sigma_matrix(&mut self, k: usize) {
        // Validate k is in range [1, dimension-1]
        if k == 0 || k >= self.dimension {
            return; // Invalid generator index
        }

        // Create the generator matrix U_k
        let mut u_k = DMatrix::identity(self.dimension, self.dimension);
        
        // Set the 2x2 block at (k-1, k) indices
        let i = k - 1; // 0-based index
        let j = k;     // 0-based index
        
        u_k[(i, i)] = Complex::new(1.0, 0.0) - self.t_param; // 1 - t
        u_k[(i, j)] = self.t_param;                            // t
        u_k[(j, i)] = Complex::new(1.0, 0.0);                  // 1
        u_k[(j, j)] = Complex::new(0.0, 0.0);                  // 0

        // Multiply: M_new = M_old * U_k
        self.burau_matrix = &self.burau_matrix * &u_k;
    }

    /// Applies the inverse generator matrix U_k^{-1} for σ_k^{-1} to the Burau matrix.
    /// 
    /// U_k^{-1} is the identity matrix except for the 2x2 block at indices (k-1, k):
    /// [0     1   ]
    /// [1/t   1-1/t]
    /// 
    /// Note: k is 1-based, so we use indices k-1 and k (0-based).
    fn apply_inverse_sigma_matrix(&mut self, k: usize) {
        // Validate k is in range [1, dimension-1]
        if k == 0 || k >= self.dimension {
            return; // Invalid generator index
        }

        // Create the inverse generator matrix U_k^{-1}
        let mut u_k_inv = DMatrix::identity(self.dimension, self.dimension);
        
        // Set the 2x2 block at (k-1, k) indices
        let i = k - 1; // 0-based index
        let j = k;     // 0-based index
        
        let one_over_t = Complex::new(1.0, 0.0) / self.t_param;
        
        u_k_inv[(i, i)] = Complex::new(0.0, 0.0);             // 0
        u_k_inv[(i, j)] = Complex::new(1.0, 0.0);             // 1
        u_k_inv[(j, i)] = one_over_t;                          // 1/t
        u_k_inv[(j, j)] = Complex::new(1.0, 0.0) - one_over_t; // 1 - 1/t

        // Multiply: M_new = M_old * U_k^{-1}
        self.burau_matrix = &self.burau_matrix * &u_k_inv;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Generator;
    use nalgebra::DMatrix;

    #[test]
    fn test_initial_state() {
        let state = FingerprintState::new(4);
        assert_eq!(state.writhe, 0);
        assert_eq!(state.crossing_count, 0);
        assert_eq!(state.jones_poly_cache, None);
        assert_eq!(state.dimension, 4);
        
        // Burau matrix should be identity
        let identity = DMatrix::identity(4, 4);
        assert_eq!(state.burau_matrix, identity);
    }

    #[test]
    fn test_update_sigma() {
        let mut state = FingerprintState::new(4);
        state.update(&Generator::Sigma(1));
        assert_eq!(state.writhe, 1);
        assert_eq!(state.crossing_count, 1);
        // Burau matrix should no longer be identity
        let identity = DMatrix::identity(4, 4);
        assert_ne!(state.burau_matrix, identity);
    }

    #[test]
    fn test_update_inverse_sigma() {
        let mut state = FingerprintState::new(4);
        state.update(&Generator::InverseSigma(1));
        assert_eq!(state.writhe, -1);
        assert_eq!(state.crossing_count, 1);
        // Burau matrix should no longer be identity
        let identity = DMatrix::identity(4, 4);
        assert_ne!(state.burau_matrix, identity);
    }

    #[test]
    fn test_update_mixed() {
        let mut state = FingerprintState::new(4);
        state.update(&Generator::Sigma(1));
        state.update(&Generator::Sigma(2));
        state.update(&Generator::InverseSigma(1));
        assert_eq!(state.writhe, 1); // +1 +1 -1 = 1
        assert_eq!(state.crossing_count, 3);
    }

    #[test]
    fn test_reset() {
        let mut state = FingerprintState::new(4);
        state.update(&Generator::Sigma(1));
        state.update(&Generator::Sigma(2));
        state.reset();
        assert_eq!(state.writhe, 0);
        assert_eq!(state.crossing_count, 0);
        // Burau matrix should be reset to identity
        let identity = DMatrix::identity(4, 4);
        assert_eq!(state.burau_matrix, identity);
    }

    #[test]
    fn test_sigma_inverse_cancellation() {
        // σ_1 * σ_1^{-1} should approximately return to identity
        let mut state = FingerprintState::new(4);
        state.update(&Generator::Sigma(1));
        state.update(&Generator::InverseSigma(1));
        
        // Due to floating point precision, we check if it's close to identity
        let identity = DMatrix::identity(4, 4);
        let diff = &state.burau_matrix - &identity;
        let max_diff = diff.iter().map(|c| c.norm()).fold(0.0, f64::max);
        // Should be very close to identity (within floating point error)
        assert!(max_diff < 1e-10, "Matrix should be close to identity after cancellation");
    }
}
