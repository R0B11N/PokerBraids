# Braid Fingerprint Visualization

This directory contains Python tools for visualizing the topological invariants computed by the braid-engine.

## Setup

Install the required Python dependencies:

```bash
pip install -r requirements.txt
```

**Note:** For the 3D visualizer, you'll need:
- PyQt5 (GUI framework)
- pyqtgraph (OpenGL plotting)
- numpy (numerical operations)

These are included in `requirements.txt`.

## Usage

### CLI Mode (File Processing)

Process a CSV file and visualize the results:

```bash
# PokerNow format
cargo run --quiet --release --bin poker-braids -- --format pokernow pokernow_sample.csv | python analysis/visualizer.py

# With reset on fold
cargo run --quiet --release --bin poker-braids -- --format pokernow --reset-on-fold pokernow_sample.csv | python analysis/visualizer.py

# Generic CSV format
cargo run --quiet --release --bin poker-braids -- sample_hand.csv | python analysis/visualizer.py
```

### Server Mode (Real-Time WebSocket)

Start the web server:

```bash
cargo run --bin poker-braids -- --server
# Or with reset on fold:
cargo run --bin poker-braids -- --server --reset-on-fold
```

The server will start on `http://127.0.0.1:3030` with two endpoints:
- `POST /action` - Accepts action strings and returns fingerprint updates
- `GET /ws` - WebSocket endpoint for real-time updates

### Live Replay Testing

Use the Python replay script to simulate a live game:

```bash
# Terminal 1: Start the server
cargo run --bin poker-braids -- --server

# Terminal 2: Replay actions
python analysis/live_replay.py pokernow_sample.csv
```

The replay script sends actions to the server with 1-second delays between actions.

## Output

The visualizer generates `braid_fingerprint.png` with:

- **Left Y-axis (Green)**: Writhe - step function showing integer crossings
- **Right Y-axis (Cyan)**: Burau Trace Magnitude - continuous line showing matrix trace evolution
- **X-axis Annotations (Magenta)**: Action descriptions rotated 45° for readability
- **Dark Theme**: Cyberpunk/hacker aesthetic with black background and neon colors

## Interpreting the Plot

- **Writhe spikes**: Indicate rapid changes in crossing direction (aggressive betting)
- **Burau magnitude changes**: Show topological complexity evolution
- **Flat periods**: May indicate passive play or trivial braids
- **Reset points**: When using `--reset-on-fold`, folds reset the state to identity

## 3D Phase Space Visualizer

For an immersive 3D visualization of the topological evolution:

```bash
# Start the Rust server first (Terminal 1)
cargo run --bin poker-braids -- --server

# Start the 3D visualizer (Terminal 2)
python analysis/visualizer_3d.py
```

The 3D visualizer renders:
- **X-axis**: Time (Step progression)
- **Y-axis**: Aggression (Writhe)
- **Z-axis**: Complexity (Burau Trace Magnitude)
- **Dynamic coloring**: Green (low complexity) → Cyan (high complexity)
- **Auto-reset**: Detects hand resets and clears the trajectory

## Real-Time WebSocket Mode

The 2D visualizer supports real-time updates via WebSocket:

```bash
# Start the Rust server first (Terminal 1)
cargo run --bin poker-braids -- --server

# Start the live visualizer (Terminal 2)
python analysis/visualizer.py --ws ws://127.0.0.1:3030/ws

# Replay actions (Terminal 3)
python analysis/live_replay.py pokernow_sample.csv
```

The visualizer will:
- Connect to the WebSocket server
- Update the plot in real-time as actions arrive
- Auto-scroll to show the last 50 steps
- Display the current action in the title
- Use dynamic scaling for both axes

## Full Stack Test

Complete end-to-end test sequence:

**Terminal 1: Start Rust Server**
```bash
cargo run --bin poker-braids -- --server
```

**Terminal 2: Start Live Visualizer**
```bash
python analysis/visualizer.py --ws ws://127.0.0.1:3030/ws
```

**Terminal 3: Replay Actions**
```bash
python analysis/live_replay.py pokernow_sample.csv
```

**Expected Result:**
- Terminal 1 shows server logs
- Terminal 2 opens a black window with neon green/cyan lines updating in real-time
- Terminal 3 shows action replay progress
- The plot updates every 100ms as new fingerprint data arrives

## Live Play with PokerNow.club

Connect the system to a live PokerNow game:

### Step 1: Start the Server

```bash
cargo run --bin poker-braids -- --server
```

### Step 2: Start the Live Visualizer

```bash
python analysis/visualizer.py --ws ws://127.0.0.1:3030/ws
```

### Step 3: Inject the Browser Script

1. Open PokerNow.club in Chrome and join/create a game
2. Open Developer Console (F12 or Right-click → Inspect → Console)
3. Open the file `analysis/injector.js` and copy its entire contents
4. Paste into the browser console and press Enter

You should see:
```
[Braid Engine] Script loaded. Waiting for DOM...
[Braid Engine] Found log container: ...
[Braid Engine] ✓ Observer started, monitoring for game actions...
```

### Step 4: Play Poker

As actions occur in the game, you'll see:
- Console messages: `[Braid Engine] ✓ Sent: ...`
- Real-time updates in the visualizer window
- Topological fingerprint evolution as the hand progresses

### How It Works

The injector script:
- Uses `MutationObserver` to watch for new log entries
- Filters for game actions (folds, checks, calls, bets, raises)
- Sends actions to `http://localhost:3030/action` via POST
- Includes debouncing to prevent duplicate sends
- Logs all activity to the console for debugging

## Troubleshooting

If the plot doesn't display:
- The script will still save `braid_fingerprint.png` even without an interactive backend
- Check that matplotlib is installed: `pip install matplotlib`
- On headless systems, the plot is saved but not displayed

If WebSocket connection fails:
- Ensure the Rust server is running: `cargo run --bin poker-braids -- --server`
- Check the WebSocket URL: `ws://127.0.0.1:3030/ws`
- Verify websockets package is installed: `pip install websockets`

If browser injector doesn't work:
- Check browser console for error messages
- Verify the server is running and accessible: `curl http://localhost:3030/action`
- Ensure CORS is enabled (should be automatic with current server config)
- Try refreshing the PokerNow page and re-injecting the script
- Check that the log container is found (look for `[Braid Engine] Found log container` message)

