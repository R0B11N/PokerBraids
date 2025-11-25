#!/usr/bin/env python3
"""
Braid Fingerprint 3D Visualizer (Phase Space)

Renders the topological evolution of a poker hand as a 3D trajectory.

Axis:
X: Time (Step)
Y: Aggression (Writhe)
Z: Complexity (Burau Magnitude)
"""
# 1. Force OpenGL import before PyQt to prevent driver conflicts on Windows
import OpenGL.GL as gl 
import sys
import json
import queue
import threading
import asyncio
import numpy as np
from websockets import client as ws_client

# GUI & 3D Imports
from PyQt5.QtWidgets import QApplication, QMainWindow, QWidget, QVBoxLayout, QLabel
from PyQt5.QtCore import QTimer, Qt
from PyQt5.QtGui import QFont
import pyqtgraph as pg
import pyqtgraph.opengl as pgl  # Renamed to 'pgl' to avoid namespace collision

# --- Configuration ---
WS_URL = "ws://127.0.0.1:3030/ws"
REFRESH_RATE_MS = 50
SCALING_X = 1.0  # Time stretch
SCALING_Y = 2.0  # Writhe height
SCALING_Z = 2.0  # Burau depth

# --- Global State ---
data_queue = queue.Queue()


class BraidWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Braid Engine: Topological Phase Space")
        self.resize(1200, 800)
        self.setStyleSheet("background-color: #050505;")

        # Central Widget
        central_widget = QWidget()
        self.setCentralWidget(central_widget)
        layout = QVBoxLayout(central_widget)
        layout.setContentsMargins(0, 0, 0, 0)

        # 1. Header / HUD
        self.hud_label = QLabel("WAITING FOR SIGNAL...")
        self.hud_label.setFont(QFont("Consolas", 16, QFont.Bold))
        self.hud_label.setStyleSheet("color: #00ff41; padding: 10px; background: rgba(0,0,0,0.8);")
        self.hud_label.setAlignment(Qt.AlignCenter)
        layout.addWidget(self.hud_label)

        # 2. 3D Viewport
        self.view = pgl.GLViewWidget()
        self.view.setBackgroundColor(pg.mkColor(5, 5, 5)) # Deep black
        self.view.setCameraPosition(distance=40, elevation=30, azimuth=-90)
        layout.addWidget(self.view)

        # 3D Objects
        self.init_3d_scene()

        # Data Buffers: Per-Player Tracking
        # Key 0 = Global, Keys 1-9 = Individual Seats
        self.history_data = {}
        for seat_id in range(10):  # 0-9 (0 = global, 1-9 = seats)
            self.history_data[seat_id] = {
                'w': [],  # writhe
                'b': []   # burau
            }
        
        # Player names cache (seat_id -> name)
        self.player_names = {}
        
        # View Mode: 0 = Global, 1-9 = Seat ID
        self.current_view_mode = 0
        
        # Initialize empty arrays with correct types for OpenGL
        self.pos_array = np.zeros((0, 3), dtype=np.float32)

        # Animation Loop
        self.timer = QTimer()
        self.timer.timeout.connect(self.update_loop)
        self.timer.start(REFRESH_RATE_MS)

    def init_3d_scene(self):
        # Grid Floor (The "Table")
        grid = pgl.GLGridItem()
        grid.scale(2, 2, 1)
        grid.setSpacing(1, 1, 1)
        grid.setColor(pg.mkColor(0, 255, 65, 80)) # Matrix Green Grid
        self.view.addItem(grid)

        # The Trajectory Line (The "Knot")
        # Initialize with dummy float32 data to set types correctly immediately
        self.line_item = pgl.GLLinePlotItem(
            pos=np.array([[0,0,0], [0,0,0]], dtype=np.float32), 
            color=pg.mkColor(0, 255, 255, 255), 
            width=3.0, 
            antialias=True
        )
        self.view.addItem(self.line_item)

        # The Head (Current State Marker)
        self.head_marker = pgl.GLScatterPlotItem(
            pos=np.array([[0,0,0]], dtype=np.float32), 
            color=[1.0, 0.0, 1.0, 1.0], 
            size=15, 
            pxMode=True
        )
        self.view.addItem(self.head_marker)

        # Zero Plane Reference
        zero_line = pgl.GLLinePlotItem(
            pos=np.array([[-100, 0, 0], [100, 0, 0]], dtype=np.float32),
            color=pg.mkColor(50, 50, 50, 255),
            width=1
        )
        self.view.addItem(zero_line)

    def update_loop(self):
        """Consume queue and update 3D geometry"""
        updated = False
        
        while not data_queue.empty():
            try:
                data = data_queue.get_nowait()
                if data is None: continue
                
                step = data['step']
                action = data['action']
                
                # Extract global metrics
                global_metrics = data.get('global', {})
                global_writhe = global_metrics.get('writhe', 0)
                global_burau = global_metrics.get('burau', 0.0)
                
                # Extract player metrics
                players = data.get('players', {})
                
                # Detect Hand Reset (Step count drops)
                if len(self.history_data[0]['w']) > 0 and step < len(self.history_data[0]['w']):
                    self.reset_trace()

                # Update Global (seat_id = 0)
                self.history_data[0]['w'].append(global_writhe)
                self.history_data[0]['b'].append(global_burau)
                
                # Update Per-Player metrics
                for seat_str, player_data in players.items():
                    try:
                        seat_id = int(seat_str)
                        if 1 <= seat_id <= 9:
                            # Store player name
                            if 'name' in player_data:
                                self.player_names[seat_id] = player_data['name']
                            
                            # Ensure lists are long enough (pad with last value if needed)
                            while len(self.history_data[seat_id]['w']) < len(self.history_data[0]['w']) - 1:
                                last_w = self.history_data[seat_id]['w'][-1] if self.history_data[seat_id]['w'] else 0
                                last_b = self.history_data[seat_id]['b'][-1] if self.history_data[seat_id]['b'] else 0.0
                                self.history_data[seat_id]['w'].append(last_w)
                                self.history_data[seat_id]['b'].append(last_b)
                            
                            self.history_data[seat_id]['w'].append(player_data.get('writhe', 0))
                            self.history_data[seat_id]['b'].append(player_data.get('complexity', 0.0))
                    except (ValueError, KeyError):
                        continue
                
                # Update HUD with current view mode
                self.update_hud(action, step)
                updated = True
            except (queue.Empty, KeyError):
                break

        if updated:
            self.redraw_trace()
    
    def update_hud(self, action: str, step: int):
        """Update HUD label based on current view mode"""
        if self.current_view_mode == 0:
            # Global view
            w = self.history_data[0]['w'][-1] if self.history_data[0]['w'] else 0
            b = self.history_data[0]['b'][-1] if self.history_data[0]['b'] else 0.0
            self.hud_label.setText(f"WATCHING: GLOBAL\nLIVE: {action}\n[Writhe: {w} | Burau: {b:.2f}]")
        else:
            # Player view
            seat_id = self.current_view_mode
            player_name = self.player_names.get(seat_id, f"Seat {seat_id}")
            if seat_id in self.history_data and self.history_data[seat_id]['w']:
                w = self.history_data[seat_id]['w'][-1]
                b = self.history_data[seat_id]['b'][-1]
                self.hud_label.setText(f"WATCHING: SEAT {seat_id} ({player_name})\nLIVE: {action}\n[Writhe: {w} | Complexity: {b:.2f}]")
            else:
                self.hud_label.setText(f"WATCHING: SEAT {seat_id} ({player_name})\nLIVE: {action}\n[No data yet]")

    def reset_trace(self):
        """Clear the 3D line for a new hand"""
        for seat_id in range(10):
            self.history_data[seat_id]['w'] = []
            self.history_data[seat_id]['b'] = []
        # Note: Don't clear player_names, as they persist across hands
        # Clear with empty float32 array
        self.pos_array = np.zeros((0, 3), dtype=np.float32)
        self.line_item.setData(pos=self.pos_array)
        print("--- HAND RESET ---")

    def keyPressEvent(self, event):
        """Handle hotkey presses to switch view modes"""
        key = event.key()
        # Qt.Key_0 through Qt.Key_9
        if key >= Qt.Key_0 and key <= Qt.Key_9:
            seat_id = key - Qt.Key_0
            self.current_view_mode = seat_id
            self.redraw_trace()
            # Update title and HUD
            if seat_id == 0:
                self.setWindowTitle("Braid Engine: Topological Phase Space (GLOBAL)")
            else:
                player_name = self.player_names.get(seat_id, f"Seat {seat_id}")
                self.setWindowTitle(f"Braid Engine: Topological Phase Space (SEAT {seat_id}: {player_name})")
            # Refresh HUD to show updated view mode
            if self.history_data[0]['w']:
                action = "Current View"
                self.update_hud(action, len(self.history_data[0]['w']))
        super().keyPressEvent(event)
    
    def redraw_trace(self):
        # Get data for current view mode
        view_data = self.history_data.get(self.current_view_mode, {'w': [], 'b': []})
        writhe_list = view_data['w']
        burau_list = view_data['b']
        
        if not writhe_list or not burau_list:
            return

        # Prepare 3D Coordinates
        # X = Time (step index), Y = Writhe, Z = Burau
        steps = np.arange(len(writhe_list))
        x = steps * SCALING_X
        y = np.array(writhe_list) * SCALING_Y
        z = np.array(burau_list) * SCALING_Z

        # Center X around the current head to keep camera focused
        center_x = x[-1]
        x_centered = x - center_x

        # CRITICAL FIX FOR WINDOWS OPENGL:
        # 1. Cast to float32
        # 2. Force memory to be contiguous (C-style) using ascontiguousarray
        pos_data = np.column_stack((x_centered, y, z)).astype(np.float32)
        self.pos_array = np.ascontiguousarray(pos_data)

        # Dynamic Coloring based on Burau (Complexity)
        # Low = Green, High = Cyan/Pink
        colors = np.zeros((len(z), 4), dtype=np.float32)
        
        # Vectorized color calculation for speed
        # Normalize roughly based on max expected burau (e.g. 24.0)
        intensity = np.clip(z / 24.0, 0.0, 1.0).astype(np.float32)
        
        colors[:, 0] = 0.0              # R
        colors[:, 1] = 1.0 - intensity  # G (Fade out green as complexity rises)
        colors[:, 2] = 1.0              # B
        colors[:, 3] = 1.0              # Alpha

        # Force contiguous memory for colors too
        colors_contiguous = np.ascontiguousarray(colors)

        # Update Geometry
        self.line_item.setData(pos=self.pos_array, color=colors_contiguous)
        
        # Update Head
        if len(self.pos_array) > 0:
            last_pos = self.pos_array[-1]
            # Reshape for scatter plot (1, 3)
            head_pos = np.ascontiguousarray(last_pos.reshape(1, 3))
            self.head_marker.setData(pos=head_pos, color=[1.0, 0.0, 1.0, 1.0])


# --- WebSocket Threading ---
def start_websocket_thread():
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    loop.run_until_complete(ws_listen())


async def ws_listen():
    while True:
        try:
            async with ws_client.connect(WS_URL, ping_interval=None) as websocket:
                print("Connected to Braid Engine.")
                async for message in websocket:
                    data_queue.put(json.loads(message))
        except Exception as e:
            print(f"Connection error: {e}. Retrying in 2s...")
            await asyncio.sleep(2)


# --- Main Entry Point ---
if __name__ == "__main__":
    # 1. Start WebSocket Consumer in Background
    ws_thread = threading.Thread(target=start_websocket_thread, daemon=True)
    ws_thread.start()

    # 2. Start GUI
    app = QApplication(sys.argv)
    window = BraidWindow()
    window.show()
    sys.exit(app.exec_())

