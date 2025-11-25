# Braids at the Table

A high-performance poker player profiling system that maps betting sequences to topological braids, providing real-time topological invariants for poker hand analysis.

## Overview

This project implements a complete pipeline from poker action logs to topological braid analysis:

1. **Parser**: Extracts actions from CSV logs (generic or PokerNow format)
2. **Braid Engine**: Maps actions to Artin generators and computes topological invariants
3. **Server**: Real-time WebSocket server for live HUD functionality
4. **Visualizer**: Python tool for plotting topological fingerprint evolution
5. **Browser Bridge**: JavaScript injector for live PokerNow.club integration

## Architecture

```
PokerNow.club → [Browser Injector] → Rust Server → WebSocket → Python Visualizer
     ↓
  CSV Logs → Parser → Braid Engine → Topological Invariants
```

## Quick Start

### Prerequisites

- Rust (latest stable)
- Python 3.7+
- Chrome browser (for live play)

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd PokerBraids

# Install Python dependencies
pip install -r analysis/requirements.txt

# Build Rust project
cargo build --release
```

## Usage

### Live Play (Recommended)

**Terminal 1: Start Server**
```bash
cargo run --bin poker-braids -- --server
```

**Terminal 2: Start Visualizer**
```bash
python analysis/visualizer.py --ws ws://127.0.0.1:3030/ws
```

**Browser:**
1. Open PokerNow.club and join a game
2. Open Developer Console (F12)
3. Copy and paste contents of `analysis/injector.js`
4. Press Enter

Watch the topological fingerprint evolve in real-time as you play!

### Offline Analysis

Process a CSV file:

```bash
# PokerNow format
cargo run --bin poker-braids -- --format pokernow pokernow_sample.csv | python analysis/visualizer.py

# Generic format
cargo run --bin poker-braids -- sample_hand.csv | python analysis/visualizer.py
```

## Project Structure

```
PokerBraids/
├── braid-engine/      # Core topological braid library
│   ├── src/
│   │   ├── types.rs           # Domain types (Seat, Action, Generator, BraidWord)
│   │   ├── mapping.rs         # Action → Generator expansion
│   │   ├── invariants.rs      # Tiered fingerprint system (Writhe, Burau)
│   │   └── normalization.rs   # Braid word reduction
│   └── tests/
│       └── toy_hand.rs        # Integration tests
├── poker-parser/      # CSV and PokerNow log parsing
│   ├── src/
│   │   ├── lib.rs             # Generic CSV parser
│   │   └── pokernow.rs        # PokerNow regex parser
├── hud-bridge/        # Web server and CLI
│   ├── src/
│   │   ├── main.rs            # Entry point (CLI/Server mode)
│   │   ├── cli.rs             # File processing mode
│   │   └── server.rs          # WebSocket server
└── analysis/          # Python visualization tools
    ├── visualizer.py          # Plotting (STDIN/WebSocket modes)
    ├── live_replay.py         # Action replay script
    ├── injector.js            # Browser console script
    └── requirements.txt       # Python dependencies
```

## Topological Invariants

The system computes three tiers of invariants:

- **Tier 1 (Instant)**: Writhe, Crossing Count - O(1) integer arithmetic
- **Tier 2 (Fast)**: Burau Trace Magnitude - O(N²) matrix operations
- **Tier 3 (Slow)**: Jones Polynomial - Computed on-demand only

The Burau Trace Magnitude provides a scalar metric suitable for HUD display, representing the "energy" or "complexity" of the hand's betting pattern.

## Features

- ✅ Real-time WebSocket updates
- ✅ Browser integration (PokerNow.club)
- ✅ Dual-mode visualizer (offline/online)
- ✅ CORS-enabled server
- ✅ Thread-safe shared state
- ✅ Auto-scrolling plots
- ✅ Dynamic axis scaling
- ✅ Cyberpunk aesthetic visualization

## Documentation

- [Analysis Tools README](analysis/README.md) - Visualization and replay tools
- [Quick Start Guide](analysis/QUICK_START.md) - Step-by-step testing instructions

## Testing

Run all tests:

```bash
cargo test --workspace
```

## License

[Your License Here]

## Contributing

[Contributing Guidelines]

