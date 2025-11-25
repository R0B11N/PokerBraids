#!/usr/bin/env python3
"""
Braid Fingerprint Visualizer

Supports two modes:
1. STDIN mode: Reads JSON stream from STDIN and plots once (offline)
2. WebSocket mode: Connects to ws://127.0.0.1:3030/ws and updates plot in real-time

Designed with a cyberpunk/hacker aesthetic: dark background, neon lines.
"""

import json
import sys
import argparse
import queue
import threading
import asyncio
import re
import pandas as pd
import matplotlib.pyplot as plt
import matplotlib.animation as animation
import matplotlib.patches as mpatches
from websockets import client as ws_client

# Colors: Neon green for writhe, cyan for burau magnitude
WRITHE_COLOR = '#00ff41'  # Matrix green
BURAU_COLOR = '#00ffff'   # Cyan
ACTION_COLOR = '#ff00ff'  # Magenta for annotations

# Global data storage for real-time mode
data_queue = queue.Queue()
data_lock = threading.Lock()
history_buffer = []  # List of full JSON objects
view_mode = 0  # 0 = Global, 1-10 = Seat ID
player_registry = {}  # Maps Seat ID (str) -> Player Name (str) - Persistent across frames

# Plot objects (initialized in setup_plot)
fig = None
ax1 = None
ax2 = None
writhe_line = None
burau_line = None
title_text = None
roster_text = None  # Player roster HUD

# Configuration
WINDOW_SIZE = 50  # Show last N steps in auto-scroll mode
AUTO_SCROLL = True  # Enable auto-scroll when data exceeds window


def on_key_press(event):
    """Handle keyboard hotkeys to switch view modes using static [S#] tags"""
    global view_mode, title_text, player_registry
    
    key = event.key
    # Map '`' (backtick) or 'g' to Global View
    if key == '`' or key == 'g' or key == 'G':
        view_mode = 0
        title_text.set_text('WATCHING: GLOBAL')
        print("Switched to GLOBAL view", file=sys.stderr)
    # Map number keys (0-9) to seats using static [S#] tags
    elif key in '1234567890':
        # Determine target seat tag (0 -> [S10], 1-9 -> [S1]-[S9])
        target_tag = '[S10]' if key == '0' else f'[S{key}]'
        
        # Find seat ID where player name starts with target tag
        found_seat = None
        for seat_str, player_name in player_registry.items():
            if isinstance(player_name, str) and player_name.startswith(target_tag):
                found_seat = seat_str
                break
        
        # Safety: If no match found, print message and don't change view
        if not found_seat:
            print(f"No player found for hotkey '{key}' (looking for {target_tag})", file=sys.stderr)
        else:
            view_mode = int(found_seat)
            player_name = player_registry.get(found_seat, f"Seat {found_seat}")
            title_text.set_text(f'WATCHING: SEAT {found_seat} ({player_name})')
            print(f"Switched to SEAT {found_seat} ({player_name}) view", file=sys.stderr)
    # Trigger plot refresh by updating the figure
    fig.canvas.draw_idle()


def setup_plot():
    """Initialize the matplotlib plot with dark theme"""
    global fig, ax1, ax2, writhe_line, burau_line, title_text, roster_text
    
    plt.style.use('dark_background')
    fig, ax1 = plt.subplots(figsize=(14, 8))
    fig.patch.set_facecolor('#000000')
    ax1.set_facecolor('#0a0a0a')
    
    # Connect key press event
    fig.canvas.mpl_connect('key_press_event', on_key_press)
    
    # Left Y-axis: Writhe (step function)
    ax1.set_xlabel('Step', color='white', fontsize=12, fontweight='bold')
    ax1.set_ylabel('Writhe', color=WRITHE_COLOR, fontsize=12, fontweight='bold')
    ax1.tick_params(axis='y', labelcolor=WRITHE_COLOR)
    ax1.tick_params(axis='x', labelcolor='white')
    ax1.grid(True, alpha=0.3, color='#333333', linestyle='--')
    
    # Initialize empty lines
    writhe_line, = ax1.step([], [], where='post', color=WRITHE_COLOR, 
                            linewidth=2.5, label='Writhe', alpha=0.9)
    
    # Right Y-axis: Burau Trace Magnitude (continuous line)
    ax2 = ax1.twinx()
    ax2.set_ylabel('Burau Trace Magnitude', color=BURAU_COLOR, fontsize=12, fontweight='bold')
    ax2.tick_params(axis='y', labelcolor=BURAU_COLOR)
    
    burau_line, = ax2.plot([], [], color=BURAU_COLOR, linewidth=2.5,
                          label='Burau Trace Magnitude', alpha=0.9,
                          marker='o', markersize=4)
    
    # Title
    title_text = fig.suptitle('WATCHING: GLOBAL', 
                              fontsize=16, fontweight='bold',
                              color='white', y=0.98)
    
    # Legend
    writhe_patch = mpatches.Patch(color=WRITHE_COLOR, label='Writhe')
    burau_line_legend = plt.Line2D([0], [0], color=BURAU_COLOR, 
                                    linewidth=2.5, label='Burau Trace Magnitude')
    ax1.legend(handles=[writhe_patch, burau_line_legend], 
               loc='upper left',
               facecolor='#1a1a1a',
               edgecolor='#333333',
               labelcolor='white')
    
    # UI Layout Fix: Create sidebar for roster
    plt.subplots_adjust(right=0.75, left=0.05)  # Reserve right 25% for roster, add left margin
    
    # Player Roster HUD (positioned in the sidebar)
    roster_text = fig.text(0.77, 0.95, 'Waiting for players...',
                          fontsize=9, color='#00ff41', alpha=0.9,
                          verticalalignment='top', horizontalalignment='left',
                          family='monospace',
                          transform=fig.transFigure)
    
    # Instructions text
    instructions = fig.text(0.02, 0.02, 'Hotkeys: `/g=Global, 0-9=Seats (by [S#] tag)',
                            fontsize=10, color='#888888', alpha=0.7)
    
    plt.tight_layout()
    
    return fig, ax1, ax2, writhe_line, burau_line, title_text


async def websocket_consumer(ws_url):
    """Async function to consume WebSocket messages and put them in queue"""
    try:
        # Disable ping_interval to prevent keepalive timeout errors
        # This is necessary for long idle periods (e.g., spectating mode)
        async with ws_client.connect(ws_url, ping_interval=None) as websocket:
            print(f"Connected to {ws_url}", file=sys.stderr)
            async for message in websocket:
                try:
                    data = json.loads(message)
                    data_queue.put(data)
                except json.JSONDecodeError as e:
                    print(f"Warning: Invalid JSON received: {e}", file=sys.stderr)
                    continue
    except Exception as e:
        print(f"WebSocket error: {e}", file=sys.stderr)
        # Put a sentinel value to signal end
        data_queue.put(None)


def websocket_thread(ws_url):
    """Thread wrapper for WebSocket connection"""
    asyncio.set_event_loop(asyncio.new_event_loop())
    loop = asyncio.get_event_loop()
    loop.run_until_complete(websocket_consumer(ws_url))


def update_plot(frame):
    """Animation callback to update plot from queue"""
    global history_buffer, view_mode, player_registry
    
    # Ingest: Pull from data_queue and append to history_buffer
    updated = False
    while True:
        try:
            data = data_queue.get_nowait()
            if data is None:  # Sentinel value
                return
            
            with data_lock:
                # Reset Logic: If step drops (reset), clear history_buffer
                # CRITICAL: Do NOT clear player_registry on reset - names persist across hands
                # player_registry is NEVER cleared, ensuring roster stability
                if len(history_buffer) > 0 and 'step' in data:
                    last_step = history_buffer[-1].get('step', 0)
                    if data['step'] < last_step:
                        history_buffer = []
                        # player_registry remains intact - DO NOT CLEAR
                        print("--- HAND RESET ---", file=sys.stderr)
                
                # Update Persistent Player Registry
                # Merge incoming player data into registry (always overwrite to catch name updates)
                # This ensures player names update when [S#] tags appear or players change seats
                # Expected format: player_data['name'] = "[S9] Barmom @ gmuSM0e3Nz" or similar
                players = data.get('players', {})
                if players:
                    for seat_str, player_data in players.items():
                        if isinstance(player_data, dict) and 'name' in player_data:
                            # Always overwrite to catch name updates (including [S#] tag additions)
                            player_registry[seat_str] = player_data['name']
                
                history_buffer.append(data)
                updated = True
        except queue.Empty:
            break
    
    if not updated and len(history_buffer) == 0:
        return  # No data yet
    
    # Reconstruct Arrays: Iterate through history_buffer based on view_mode
    with data_lock:
        steps = []
        writhe = []
        burau = []
        current_action = 'Waiting for data...'
        
        for item in history_buffer:
            step = item.get('step', 0)
            action = item.get('action', '')
            
            if view_mode == 0:
                # Global view: Use global metrics
                global_metrics = item.get('global', {})
                if not global_metrics:
                    # Backward compatibility: old format
                    writhe_val = item.get('writhe', 0)
                    burau_val = item.get('burau_trace_magnitude', 0.0)
                else:
                    writhe_val = global_metrics.get('writhe', 0)
                    burau_val = global_metrics.get('burau', 0.0)
            else:
                # Seat view: Look up player in players map (supports seats 1-10)
                players = item.get('players', {})
                seat_str = str(view_mode)
                player_data = players.get(seat_str, {})
                
                if player_data:
                    writhe_val = player_data.get('writhe', 0)
                    burau_val = player_data.get('complexity', 0.0)
                else:
                    # Player not in hand (folded/not present): Use 0
                    writhe_val = 0
                    burau_val = 0.0
            
            steps.append(step)
            writhe.append(writhe_val)
            burau.append(burau_val)
            current_action = action  # Keep last action
    
    if len(steps) == 0:
        return
    
    # Auto-scroll: show last WINDOW_SIZE steps
    if AUTO_SCROLL and len(steps) > WINDOW_SIZE:
        steps = steps[-WINDOW_SIZE:]
        writhe = writhe[-WINDOW_SIZE:]
        burau = burau[-WINDOW_SIZE:]
    
    # Update writhe line (step function)
    writhe_line.set_data(steps, writhe)
    
    # Update burau line
    burau_line.set_data(steps, burau)
    
    # Dynamic scaling (auto-rescale when switching views)
    if len(steps) > 0:
        ax1.set_xlim([min(steps) - 0.5, max(steps) + 0.5])
        if len(writhe) > 0:
            writhe_min = min(writhe)
            writhe_max = max(writhe)
            # Add padding to prevent flat lines from being invisible
            if writhe_max == writhe_min:
                ax1.set_ylim([writhe_min - 1, writhe_max + 1])
            else:
                ax1.set_ylim([writhe_min - 1, writhe_max + 1])
        
        if len(burau) > 0:
            burau_min = min(burau)
            burau_max = max(burau)
            # Add padding to prevent flat lines from being invisible
            if burau_max == burau_min:
                margin = 0.1
            else:
                margin = (burau_max - burau_min) * 0.1
            ax2.set_ylim([burau_min - margin, burau_max + margin])
    
    # Update title with current view mode and action
    if view_mode == 0:
        title_text.set_text(f'WATCHING: GLOBAL - {current_action}')
    else:
        # Get player name from persistent registry
        seat_str = str(view_mode)
        player_name = player_registry.get(seat_str, f"Seat {view_mode}")
        title_text.set_text(f'WATCHING: SEAT {view_mode} ({player_name}) - {current_action}')
    
    # Update Player Roster HUD (use persistent registry, sorted by [S#] tag)
    if player_registry:
        # Build roster string sorted by [S#] tag
        roster_lines = ['[G] GLOBAL VIEW', '']
        
        # Create list of (seat_number, seat_str, player_name) tuples
        roster_entries = []
        for seat_str, player_name in player_registry.items():
            if not isinstance(player_name, str):
                continue
            
            # Extract seat number from [S#] tag in name (search anywhere in string)
            # Regex handles single digits (1-9) and double digits (10) correctly
            seat_number = None
            if '[S' in player_name:
                # Robust regex: matches [S1], [S10], [S99], etc.
                match = re.search(r'\[S(\d+)\]', player_name)
                if match:
                    seat_number = int(match.group(1))
            
            # If no tag found, skip this entry
            if seat_number is None:
                continue
            
            roster_entries.append((seat_number, seat_str, player_name))
        
        # Sort by seat number (from [S#] tag)
        roster_entries.sort(key=lambda x: x[0])
        
        # Build roster entries with hotkey labels
        for seat_number, seat_str, player_name in roster_entries:
            # Determine hotkey label: [0] for Seat 10, [1-9] for Seats 1-9
            if seat_number == 10:
                label = '[0]'
            elif 1 <= seat_number <= 9:
                label = f'[{seat_number}]'
            else:
                label = '[?]'  # Fallback for unexpected seat numbers
            
            # Clean the name: Remove [S#] tag and @ ID part for display
            # Format: "[S9] Barmom @ gmuSM0e3Nz" -> "Barmom"
            clean_name = player_name
            # Remove [S#] tag if present
            clean_name = re.sub(r'\[S\d+\]\s*', '', clean_name)
            # Remove @ ID part if present
            if ' @ ' in clean_name:
                clean_name = clean_name.split(' @ ')[0].strip()
            
            # Display format: [hotkey] CleanName (Seat N)
            roster_lines.append(f'{label} {clean_name} (Seat {seat_number})')
        
        roster_text.set_text('\n'.join(roster_lines))
    else:
        roster_text.set_text('Waiting for players...')
    
    return writhe_line, burau_line, title_text


def plot_static(data):
    """Plot all data at once (STDIN mode)"""
    global history_buffer, view_mode, player_registry
    
    if not data:
        print("Error: No valid JSON data received", file=sys.stderr)
        sys.exit(1)
    
    # Store in history_buffer for consistency
    history_buffer = data
    
    # Populate player registry from all data points
    player_registry.clear()
    for item in data:
        players = item.get('players', {})
        if players:
            for seat_str, player_data in players.items():
                if isinstance(player_data, dict) and 'name' in player_data:
                    player_registry[seat_str] = player_data['name']
    
    # Convert to DataFrame for easier processing
    df = pd.DataFrame(data)
    df = df.sort_values('step').reset_index(drop=True)
    
    # Setup plot
    setup_plot()
    
    # Extract data based on view_mode (default: Global)
    if view_mode == 0:
        # Global view
        if 'global' in df.columns:
            # New format
            writhe_data = df['global'].apply(lambda x: x.get('writhe', 0) if isinstance(x, dict) else 0)
            burau_data = df['global'].apply(lambda x: x.get('burau', 0.0) if isinstance(x, dict) else 0.0)
        else:
            # Backward compatibility: old format
            writhe_data = df.get('writhe', pd.Series([0] * len(df)))
            burau_data = df.get('burau_trace_magnitude', pd.Series([0.0] * len(df)))
    else:
        # Seat view (supports seats 1-10)
        seat_str = str(view_mode)
        writhe_data = []
        burau_data = []
        
        for _, row in df.iterrows():
            players = row.get('players', {})
            if isinstance(players, dict) and seat_str in players:
                player = players[seat_str]
                writhe_data.append(player.get('writhe', 0))
                burau_data.append(player.get('complexity', 0.0))
            else:
                writhe_data.append(0)
                burau_data.append(0.0)
        
        writhe_data = pd.Series(writhe_data)
        burau_data = pd.Series(burau_data)
    
    # Plot writhe as step function
    ax1.step(df['step'], writhe_data, 
             where='post', 
             color=WRITHE_COLOR, 
             linewidth=2.5,
             label='Writhe',
             alpha=0.9)
    
    if len(writhe_data) > 0:
        writhe_min = writhe_data.min()
        writhe_max = writhe_data.max()
        if writhe_max == writhe_min:
            ax1.set_ylim([writhe_min - 1, writhe_max + 1])
        else:
            ax1.set_ylim([writhe_min - 1, writhe_max + 1])
    
    # Plot burau magnitude as line
    ax2.plot(df['step'], burau_data, 
             color=BURAU_COLOR, 
             linewidth=2.5,
             label='Burau Trace Magnitude',
             alpha=0.9,
             marker='o',
             markersize=4)
    
    if len(burau_data) > 0:
        burau_min = burau_data.min()
        burau_max = burau_data.max()
        if burau_max == burau_min:
            margin = 0.1
        else:
            margin = (burau_max - burau_min) * 0.1
        ax2.set_ylim([burau_min - margin, burau_max + margin])
    
    # Annotate actions on x-axis
    ylim = ax1.get_ylim()
    y_min = ylim[0]
    
    for idx, row in df.iterrows():
        step = row['step']
        action = row.get('action', '')
        
        # Truncate long action strings for readability
        action_short = action[:30] + '...' if len(action) > 30 else action
        
        # Rotate annotations 45 degrees
        ax1.text(step, y_min - (y_min * 0.1), 
                 action_short, 
                 rotation=45, 
                 ha='left',
                 va='top',
                 fontsize=8,
                 color=ACTION_COLOR,
                 alpha=0.7)
    
    # Update title
    if view_mode == 0:
        title_text.set_text('Braid Fingerprint Evolution (GLOBAL)')
    else:
        title_text.set_text(f'Braid Fingerprint Evolution (SEAT {view_mode})')
    
    # Update Player Roster HUD (use persistent registry, sorted by [S#] tag)
    if player_registry:
        # Build roster string sorted by [S#] tag
        roster_lines = ['[G] GLOBAL VIEW', '']
        
        # Create list of (seat_number, seat_str, player_name) tuples
        roster_entries = []
        for seat_str, player_name in player_registry.items():
            if not isinstance(player_name, str):
                continue
            
            # Extract seat number from [S#] tag in name (search anywhere in string)
            # Regex handles single digits (1-9) and double digits (10) correctly
            seat_number = None
            if '[S' in player_name:
                # Robust regex: matches [S1], [S10], [S99], etc.
                match = re.search(r'\[S(\d+)\]', player_name)
                if match:
                    seat_number = int(match.group(1))
            
            # If no tag found, skip this entry
            if seat_number is None:
                continue
            
            roster_entries.append((seat_number, seat_str, player_name))
        
        # Sort by seat number (from [S#] tag)
        roster_entries.sort(key=lambda x: x[0])
        
        # Build roster entries with hotkey labels
        for seat_number, seat_str, player_name in roster_entries:
            # Determine hotkey label: [0] for Seat 10, [1-9] for Seats 1-9
            if seat_number == 10:
                label = '[0]'
            elif 1 <= seat_number <= 9:
                label = f'[{seat_number}]'
            else:
                label = '[?]'  # Fallback for unexpected seat numbers
            
            # Clean the name: Remove [S#] tag and @ ID part for display
            # Format: "[S9] Barmom @ gmuSM0e3Nz" -> "Barmom"
            clean_name = player_name
            # Remove [S#] tag if present
            clean_name = re.sub(r'\[S\d+\]\s*', '', clean_name)
            # Remove @ ID part if present
            if ' @ ' in clean_name:
                clean_name = clean_name.split(' @ ')[0].strip()
            
            # Display format: [hotkey] CleanName (Seat N)
            roster_lines.append(f'{label} {clean_name} (Seat {seat_number})')
        
        roster_text.set_text('\n'.join(roster_lines))
    else:
        roster_text.set_text('Waiting for players...')
    
    # Save plot
    output_file = 'braid_fingerprint.png'
    plt.savefig(output_file, dpi=150, facecolor='#000000', edgecolor='none')
    print(f"Plot saved to {output_file}", file=sys.stderr)
    
    # Show plot
    try:
        plt.show()
    except Exception:
        print("Non-interactive backend detected, skipping display", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(
        description='Visualize braid fingerprint evolution from JSON data'
    )
    parser.add_argument(
        '--ws',
        type=str,
        default=None,
        help='WebSocket URL for real-time updates (e.g., ws://127.0.0.1:3030/ws)'
    )
    args = parser.parse_args()
    
    if args.ws:
        # WebSocket mode: Real-time plotting
        print(f"Starting live visualizer (WebSocket mode)", file=sys.stderr)
        print(f"Connecting to {args.ws}...", file=sys.stderr)
        
        # Setup plot
        setup_plot()
        
        # Start WebSocket thread
        ws_thread = threading.Thread(target=websocket_thread, args=(args.ws,), daemon=True)
        ws_thread.start()
        
        # Start animation
        ani = animation.FuncAnimation(fig, update_plot, interval=100, blit=False)
        
        # Show plot (blocking)
        try:
            plt.show()
        except KeyboardInterrupt:
            print("\nVisualizer stopped by user", file=sys.stderr)
    else:
        # STDIN mode: Read all data and plot once
        print("Reading data from STDIN...", file=sys.stderr)
        data = []
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
                data.append(obj)
            except json.JSONDecodeError as e:
                print(f"Warning: Skipping invalid JSON line: {e}", file=sys.stderr)
                continue
        
        plot_static(data)


if __name__ == "__main__":
    main()
