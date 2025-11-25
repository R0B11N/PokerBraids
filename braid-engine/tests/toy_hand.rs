use braid_engine::{
    expand_action, normalize, Action, ActionType, BraidWord, FingerprintState, Generator,
    IncrementalUpdate, Seat,
};
use nalgebra::DMatrix;

/// Integration test for the "Toy Hand" scenario from the Appendix.
/// 
/// Scenario:
/// - 4 Players
/// - Actions: Seat 1 Raise -> Seat 3 Call -> Seat 2 Raise -> Seat 4 Call -> Seat 1 Re-Raise
/// 
/// This test verifies:
/// 1. The BraidWord matches the expected expansion
/// 2. The FingerprintState correctly updates the writhe count
#[test]
fn test_toy_hand() {
    const TOTAL_SEATS: usize = 4;

    // Define the action sequence
    let actions = vec![
        Action::new(Seat::new(1), ActionType::Raise, 100),
        Action::new(Seat::new(3), ActionType::Call, 100),
        Action::new(Seat::new(2), ActionType::Raise, 200),
        Action::new(Seat::new(4), ActionType::Call, 200),
        Action::new(Seat::new(1), ActionType::ReRaise, 400),
    ];

    // Build the braid word by tracking action flow
    // The action "flows" from one seat to the next
    let mut braid_word = BraidWord::new();
    let mut current_seat = actions[0].seat;

    // Process each action: the action moves from current_seat to action.seat
    for action in &actions[1..] {
        let target_seat = action.seat;
        let generators = expand_action(current_seat, target_seat, TOTAL_SEATS);
        for gen in &generators {
            braid_word.push(*gen);
        }
        current_seat = target_seat;
    }

    // Expected expansion:
    // 1 -> 3: σ₁, σ₂
    // 3 -> 2: σ₂⁻¹
    // 2 -> 4: σ₂, σ₃
    // 4 -> 1: σ₃⁻¹, σ₂⁻¹, σ₁⁻¹
    let expected_generators = vec![
        Generator::Sigma(1),      // 1 -> 2
        Generator::Sigma(2),      // 2 -> 3
        Generator::InverseSigma(2), // 3 -> 2
        Generator::Sigma(2),      // 2 -> 3
        Generator::Sigma(3),      // 3 -> 4
        Generator::InverseSigma(3), // 4 -> 3
        Generator::InverseSigma(2), // 3 -> 2
        Generator::InverseSigma(1), // 2 -> 1
    ];

    // Verify the braid word matches expected expansion
    let actual_generators: Vec<Generator> = braid_word.iter().copied().collect();
    assert_eq!(
        actual_generators, expected_generators,
        "Braid word expansion does not match expected generators"
    );

    // Verify the fingerprint state
    let mut fingerprint = FingerprintState::new(TOTAL_SEATS);
    for gen in braid_word.iter() {
        fingerprint.update(gen);
    }

    // Calculate expected writhe:
    // σ₁: +1, σ₂: +1, σ₂⁻¹: -1, σ₂: +1, σ₃: +1, σ₃⁻¹: -1, σ₂⁻¹: -1, σ₁⁻¹: -1
    // Total: 1 + 1 - 1 + 1 + 1 - 1 - 1 - 1 = 0
    let expected_writhe = 0;
    assert_eq!(
        fingerprint.writhe, expected_writhe,
        "Writhe count does not match expected value"
    );

    // Verify crossing count
    assert_eq!(
        fingerprint.crossing_count, expected_generators.len(),
        "Crossing count should equal the number of generators"
    );

    // Verify Burau matrix is not identity (proving the hand has topological content)
    let identity = DMatrix::identity(TOTAL_SEATS, TOTAL_SEATS);
    assert_ne!(
        fingerprint.burau_matrix, identity,
        "Burau matrix should not be identity after processing a hand"
    );

    println!("✓ Toy Hand test passed!");
    println!("  Braid word length: {}", braid_word.len());
    println!("  Writhe: {}", fingerprint.writhe);
    println!("  Crossing count: {}", fingerprint.crossing_count);
}

/// Additional test to verify the action flow logic more explicitly.
#[test]
fn test_action_flow_sequence() {
    const TOTAL_SEATS: usize = 4;

    // Simulate the exact sequence from the toy hand
    // Seat 1 Raise -> Seat 3 Call -> Seat 2 Raise -> Seat 4 Call -> Seat 1 Re-Raise

    // Action 1: Seat 1 Raise (initial action, no movement)
    let mut braid_word = BraidWord::new();
    let mut current = Seat::new(1);

    // Action 2: Seat 3 Call (1 -> 3)
    let gen_1_to_3 = expand_action(current, Seat::new(3), TOTAL_SEATS);
    assert_eq!(gen_1_to_3, vec![Generator::Sigma(1), Generator::Sigma(2)]);
    for gen in &gen_1_to_3 {
        braid_word.push(*gen);
    }
    current = Seat::new(3);

    // Action 3: Seat 2 Raise (3 -> 2)
    let gen_3_to_2 = expand_action(current, Seat::new(2), TOTAL_SEATS);
    assert_eq!(gen_3_to_2, vec![Generator::InverseSigma(2)]);
    for gen in &gen_3_to_2 {
        braid_word.push(*gen);
    }
    current = Seat::new(2);

    // Action 4: Seat 4 Call (2 -> 4)
    let gen_2_to_4 = expand_action(current, Seat::new(4), TOTAL_SEATS);
    assert_eq!(gen_2_to_4, vec![Generator::Sigma(2), Generator::Sigma(3)]);
    for gen in &gen_2_to_4 {
        braid_word.push(*gen);
    }
    current = Seat::new(4);

    // Action 5: Seat 1 Re-Raise (4 -> 1)
    let gen_4_to_1 = expand_action(current, Seat::new(1), TOTAL_SEATS);
    assert_eq!(
        gen_4_to_1,
        vec![
            Generator::InverseSigma(3),
            Generator::InverseSigma(2),
            Generator::InverseSigma(1)
        ]
    );
    for gen in &gen_4_to_1 {
        braid_word.push(*gen);
    }

    // Verify total length
    assert_eq!(braid_word.len(), 8);

    // Verify writhe
    let mut fingerprint = FingerprintState::new(TOTAL_SEATS);
    for gen in braid_word.iter() {
        fingerprint.update(gen);
    }
    assert_eq!(fingerprint.writhe, 0);
}

/// Test for braid word normalization (Free Reduction).
#[test]
fn test_normalization() {
    // Create a word with adjacent inverse pairs: σ₁, σ₂, σ₂⁻¹, σ₁⁻¹
    let mut word = BraidWord::from_generators(vec![
        Generator::Sigma(1),
        Generator::Sigma(2),
        Generator::InverseSigma(2),
        Generator::InverseSigma(1),
    ]);

    // Normalize should reduce this to empty
    normalize(&mut word);
    assert!(
        word.is_empty(),
        "Word [σ₁, σ₂, σ₂⁻¹, σ₁⁻¹] should reduce to empty"
    );
}

