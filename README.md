# Braids at the Table

Real-time poker profiling through topological braid theory. Maps betting sequences to algebraic braids and computes invariants for behavioral fingerprinting.

<img width="1905" height="802" alt="9VAQ31h" src="https://github.com/user-attachments/assets/3fd20cbf-c5ea-428d-91e4-df095dd84b1f" />

## What This Does

Poker hands have structure. A bet-raise-reraise sequence is topologically different from check-check-bet. This system:

1. Parses poker logs (CSV or live PokerNow streams)
2. Maps each action to Artin braid generators
3. Computes topological invariants in real-time
4. Visualizes betting pattern "energy" as the hand evolves

Result: A scalar fingerprint of player behavior patterns.

## Architecture
```
PokerNow.club ──> Browser Injector ──> Rust Server ──> WebSocket ──> Python Viz
                         │
                    CSV Logs ──> Parser ──> Braid Engine ──> Invariants
```

## Running It

### Live HUD (The Cool Way)

**Terminal 1:**
```bash
cargo run --release --bin poker-braids -- --server
```

**Terminal 2:**
```bash
python analysis/visualizer.py --ws ws://127.0.0.1:3030/ws
# visualizer_3d is temporarily redacted but you could try it out too, only serves to model the state space at the moment
```

**Terminal 3:**
```bash
cd bridge
npm init -y
npm install puppeteer axios
node raw_wiretap.js https://www.pokernow.club/games/YOURURL
```


### Offline Analysis
```bash
# Process a hand history
cargo run --release -- sample_hand.csv | python analysis/visualizer.py

# PokerNow format
cargo run --release -- --format pokernow your_log.csv | python analysis/visualizer.py
```

## How It Works

### The Math

Each betting action maps to a braid generator:
- Fold → strand exits
- Check → strand continues
- Bet/Raise → strand crosses over others

The result is a braid word in the Artin group. We compute:

**Tier 1 (instant):** Writhe - sum of signed crossings  
**Tier 2 (fast):** Burau trace magnitude - matrix rep energy  
**Tier 3 (lazy):** Jones polynomial - full topological invariant

The Burau trace gives a real number: higher magnitude = more complex betting pattern.

## Reading the Graphs

Two metrics track each player's betting pattern:

<img width="1905" height="802" alt="9VAQ31h" src="https://github.com/user-attachments/assets/456b0e45-cdf6-436f-85a2-bcd78424f287" />

**Writhe (cyan)** - Directional twist in action flow
- High → aggressive, state-changing actions
- Low/negative → passive, folding

**Burau Trace Magnitude (green)** - Pattern complexity
- High → dynamic, unpredictable decisions
- Low → linear, predictable play

### Common Patterns

**Flat low writhe**  
Writhe negative and constant, Burau flat or declining  
→ Nit. Folds frequently, avoids confrontation

**Sudden writhe spike**  
Long flat line then sharp rise, Burau drops after  
→ Wait-and-strike. Passive until hitting a hand, then attacks

**High flat writhe**  
Writhe stays elevated, Burau forms slow staircase  
→ Steady aggression. Bets often, maintains pressure

**Burau staircase down**  
Writhe varies, Burau decreases stepwise  
→ Becoming predictable. Betting patterns simplify over time

**High Burau + oscillating writhe**  
Writhe fluctuates rapidly, Burau stays high  
→ Chaotic. Mixes aggression/passivity, bluffs often, hard to read

**Both flat**  
Neither metric moves much  
→ Rock. Tight, cautious, minimal involvement

**Both rising**  
Writhe and Burau both increasing  
→ Escalating. Growing aggression and complexity, pressing advantage or bluffing heavy

### Example

Player checks preflop (writhe flat), calls flop (small writhe bump), then jams turn for 3x pot (writhe spikes, Burau may drop if this becomes their standard pattern). Graph signature: flat → small rise → vertical spike.

### The Code
```
PokerBraids/
├── braid-engine/          # Core math (types, mapping, invariants)
├── poker-parser/          # CSV and PokerNow log parsing
├── bridge/                # Main injection engine
├── hud-bridge/            # WebSocket server + CLI (TEMPORARILY DEPRECATED (not sure if Puppeteer is the solution long term but it works!))
└── analysis/              # Python viz and browser injection
```

All the topology lives in `braid-engine`. The rest is plumbing.

## Performance

- Writhe: O(1) integer ops
- Burau: O(n²) matrix multiply (sub-millisecond for poker hands)
- Jones: O(exp) - not computed unless you ask, #P-Hard

Real-time performance on consumer hardware. WebSocket latency ~10ms.

## Install
```bash
git clone https://github.com/R0B11N/poker-braids
cd poker-braids
pip install -r analysis/requirements.txt
cargo build --release
```

Requires Rust stable + Python 3.7+.

## Tests
```bash
cargo test --workspace
```

See `braid-engine/tests/toy_hand.rs` for integration tests.

## Why This Matters

Sequential games under uncertainty (poker, trading, adversarial RL) share structure. Betting patterns encode strategy. Topological invariants give compact representations of these patterns.

Same math applies to order flow analysis - aggressive buying has different braid signatures than passive flow. Extremely high potential to turnover into heavy cross sector applications.

## STILL BUILDING:

Creating a prediction engine with ML on the strands to predict live time behaviour based on past hands and optimising the HUD to be more than a decent working prototype!

## License

MIT

## Contact

Questions? Open an issue or hit me up.
