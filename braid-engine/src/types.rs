/// Seat represents a player's position at the table.
/// Uses 1-based indexing for mathematical operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Seat(pub usize);

impl Seat {
    /// Creates a new Seat with 1-based indexing.
    /// Panics if seat is 0.
    pub fn new(seat: usize) -> Self {
        assert!(seat > 0, "Seat must be 1-based (seat > 0)");
        Seat(seat)
    }

    /// Returns the 1-based seat number.
    pub fn value(&self) -> usize {
        self.0
    }

    /// Returns the 0-based index for internal use.
    pub fn index(&self) -> usize {
        self.0 - 1
    }
}

/// Action type in poker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    Fold,
    Check,
    Call,
    Bet,
    Raise,
    ReRaise,
    AllIn,
    Reset, // Represents "starting hand" or explicit reset
}

/// An action taken by a player.
#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub seat: Seat,
    pub action_type: ActionType,
    pub amount: u64,
}

impl Action {
    pub fn new(seat: Seat, action_type: ActionType, amount: u64) -> Self {
        Action {
            seat,
            action_type,
            amount,
        }
    }
}

/// Artin generator for braid groups.
/// Sigma(i) represents σ_i (overcrossing)
/// InverseSigma(i) represents σ_i^{-1} (undercrossing)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Generator {
    Sigma(usize),
    InverseSigma(usize),
}

impl Generator {
    /// Returns the index of the generator (1-based).
    pub fn index(&self) -> usize {
        match self {
            Generator::Sigma(i) | Generator::InverseSigma(i) => *i,
        }
    }

    /// Returns true if this is an overcrossing (σ_i).
    pub fn is_overcrossing(&self) -> bool {
        matches!(self, Generator::Sigma(_))
    }

    /// Returns true if this is an undercrossing (σ_i^{-1}).
    pub fn is_undercrossing(&self) -> bool {
        matches!(self, Generator::InverseSigma(_))
    }
}

/// A braid word is a sequence of generators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BraidWord {
    generators: Vec<Generator>,
}

impl BraidWord {
    /// Creates a new empty braid word.
    pub fn new() -> Self {
        BraidWord {
            generators: Vec::new(),
        }
    }

    /// Creates a braid word from a vector of generators.
    pub fn from_generators(generators: Vec<Generator>) -> Self {
        BraidWord { generators }
    }

    /// Appends a generator to the braid word.
    pub fn push(&mut self, gen: Generator) {
        self.generators.push(gen);
    }

    /// Extends the braid word with generators from another braid word.
    pub fn extend(&mut self, other: &BraidWord) {
        self.generators.extend_from_slice(&other.generators);
    }

    /// Returns an iterator over the generators.
    pub fn iter(&self) -> impl Iterator<Item = &Generator> {
        self.generators.iter()
    }

    /// Returns the number of generators in the braid word.
    pub fn len(&self) -> usize {
        self.generators.len()
    }

    /// Returns true if the braid word is empty.
    pub fn is_empty(&self) -> bool {
        self.generators.is_empty()
    }

    /// Replaces the generators in this braid word.
    /// Used internally by normalization.
    pub(crate) fn replace_generators(&mut self, generators: Vec<Generator>) {
        self.generators = generators;
    }
}

impl Default for BraidWord {
    fn default() -> Self {
        BraidWord::new()
    }
}

impl From<Vec<Generator>> for BraidWord {
    fn from(generators: Vec<Generator>) -> Self {
        BraidWord::from_generators(generators)
    }
}

