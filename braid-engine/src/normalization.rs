use crate::types::{BraidWord, Generator};

/// Reduces a braid word by removing adjacent inverse pairs (Free Reduction).
/// 
/// This implements a simple free reduction algorithm:
/// - Removes pairs of the form σ_i · σ_i^{-1} or σ_i^{-1} · σ_i
/// - Iterates until no more reductions are possible
/// 
/// This is crucial for memory management during streaming, preventing
/// braid words from exploding in size.
/// 
/// # Arguments
/// * `word` - The braid word to reduce (modified in place)
/// 
/// # Example
/// ```
/// use braid_engine::{BraidWord, Generator, normalize};
/// 
/// let mut word = BraidWord::from_generators(vec![
///     Generator::Sigma(1),
///     Generator::Sigma(2),
///     Generator::InverseSigma(2),
///     Generator::InverseSigma(1),
/// ]);
/// normalize(&mut word);
/// assert!(word.is_empty());
/// ```
pub fn normalize(word: &mut BraidWord) {
    let mut changed = true;
    
    // Keep reducing until no more changes occur
    while changed {
        changed = false;
        let mut i = 0;
        
        // We need to work with the internal vector, so we'll reconstruct
        // For now, we'll use a simple approach: collect generators and rebuild
        let mut new_generators = Vec::new();
        let generators: Vec<Generator> = word.iter().copied().collect();
        
        while i < generators.len() {
            if i < generators.len() - 1 {
                let current = &generators[i];
                let next = &generators[i + 1];
                
                // Check if current and next are inverses
                let are_inverses = match (current, next) {
                    (Generator::Sigma(k1), Generator::InverseSigma(k2)) => k1 == k2,
                    (Generator::InverseSigma(k1), Generator::Sigma(k2)) => k1 == k2,
                    _ => false,
                };
                
                if are_inverses {
                    // Skip both (they cancel)
                    i += 2;
                    changed = true;
                    continue;
                }
            }
            
            // Keep this generator
            new_generators.push(generators[i]);
            i += 1;
        }
        
        // Rebuild the braid word
        word.replace_generators(new_generators);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BraidWord, Generator};

    #[test]
    fn test_normalize_empty() {
        let mut word = BraidWord::new();
        normalize(&mut word);
        assert!(word.is_empty());
    }

    #[test]
    fn test_normalize_simple_cancellation() {
        let mut word = BraidWord::from_generators(vec![
            Generator::Sigma(1),
            Generator::InverseSigma(1),
        ]);
        normalize(&mut word);
        assert!(word.is_empty(), "σ_1 · σ_1^{{-1}} should reduce to empty");
    }

    #[test]
    fn test_normalize_inverse_cancellation() {
        let mut word = BraidWord::from_generators(vec![
            Generator::InverseSigma(2),
            Generator::Sigma(2),
        ]);
        normalize(&mut word);
        assert!(word.is_empty(), "σ_2^{{-1}} · σ_2 should reduce to empty");
    }

    #[test]
    fn test_normalize_multiple_cancellations() {
        let mut word = BraidWord::from_generators(vec![
            Generator::Sigma(1),
            Generator::Sigma(2),
            Generator::InverseSigma(2),
            Generator::InverseSigma(1),
        ]);
        normalize(&mut word);
        assert!(word.is_empty(), "Should reduce to empty after multiple cancellations");
    }

    #[test]
    fn test_normalize_no_cancellation() {
        let mut word = BraidWord::from_generators(vec![
            Generator::Sigma(1),
            Generator::Sigma(2),
            Generator::Sigma(1),
        ]);
        let original_len = word.len();
        normalize(&mut word);
        assert_eq!(word.len(), original_len, "No cancellation should occur");
    }

    #[test]
    fn test_normalize_partial_cancellation() {
        let mut word = BraidWord::from_generators(vec![
            Generator::Sigma(1),
            Generator::Sigma(2),
            Generator::InverseSigma(2),
            Generator::Sigma(3),
        ]);
        normalize(&mut word);
        // Should reduce to: σ_1, σ_3
        assert_eq!(word.len(), 2);
        let generators: Vec<Generator> = word.iter().copied().collect();
        assert_eq!(generators[0], Generator::Sigma(1));
        assert_eq!(generators[1], Generator::Sigma(3));
    }

    #[test]
    fn test_normalize_nested_cancellations() {
        // After first pass: σ_1, σ_2, σ_2^{-1}, σ_1^{-1} -> σ_1, σ_1^{-1}
        // After second pass: empty
        let mut word = BraidWord::from_generators(vec![
            Generator::Sigma(1),
            Generator::Sigma(2),
            Generator::InverseSigma(2),
            Generator::InverseSigma(1),
        ]);
        normalize(&mut word);
        assert!(word.is_empty(), "Nested cancellations should reduce to empty");
    }

    #[test]
    fn test_normalize_complex_sequence() {
        // σ_1, σ_2, σ_1, σ_1^{-1}, σ_2^{-1}, σ_1
        // After first pass: σ_1, σ_2, σ_2^{-1}, σ_1
        // After second pass: σ_1, σ_1
        let mut word = BraidWord::from_generators(vec![
            Generator::Sigma(1),
            Generator::Sigma(2),
            Generator::Sigma(1),
            Generator::InverseSigma(1),
            Generator::InverseSigma(2),
            Generator::Sigma(1),
        ]);
        normalize(&mut word);
        assert_eq!(word.len(), 2, "Should reduce to σ_1, σ_1");
        let generators: Vec<Generator> = word.iter().copied().collect();
        assert_eq!(generators[0], Generator::Sigma(1));
        assert_eq!(generators[1], Generator::Sigma(1));
    }
}

