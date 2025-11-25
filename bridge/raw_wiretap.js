const puppeteer = require('puppeteer');
const axios = require('axios');

const TARGET_URL = process.argv[2];
const RUST_URL = "http://localhost:3030/action";

if (!TARGET_URL) {
    console.error("❌ Usage: node raw_wiretap.js <GAME_URL>");
    process.exit(1);
}

class GameEngine {
    constructor() {
        this.players = {};      // ID -> Name
        this.currentBets = {};  // ID -> Amount
        this.seatMap = {};      // PlayerID -> SeatIndex (0-9)
        this.indexToPlayer = {};  // SeatIndex (0-9) -> PlayerID (reverse map for seat swapping)
        this.playerIDsToSeats = {};  // PlayerID -> ServerIndex (robust seat tracking)
        this.lastProcessedBets = {};  // ID -> Last Emitted Amount (for deduplication)
    }

    // --- THE NAME HUNTER ---
    scanForNames(obj, visited = null) {
        if (!obj || typeof obj !== 'object') return;
        
        // Initialize visited set on first call to prevent processing same object twice
        if (visited === null) {
            visited = new Set();
        }
        
        // Prevent infinite recursion: skip if we've seen this object reference
        if (visited.has(obj)) {
            return;
        }
        visited.add(obj);

        // 0. UPDATE SEAT MAP with Seat Swapping Logic
        // Look for obj.seats array: [[seatIndex, playerID], ...]
        // This ensures when a new player sits in a seat, the old player is cleared
        if (obj.seats && Array.isArray(obj.seats)) {
            for (const [seatIndex, playerID] of obj.seats) {
                if (playerID && seatIndex >= 0 && seatIndex < 10) {
                    // Clear old player from this seat (if any)
                    const oldPlayerID = this.indexToPlayer[seatIndex];
                    if (oldPlayerID && oldPlayerID !== playerID) {
                        // Remove old player's seat mapping
                        delete this.seatMap[oldPlayerID];
                    }
                    
                    // Set new player in this seat
                    this.indexToPlayer[seatIndex] = playerID;
                    this.seatMap[playerID] = seatIndex;
                }
            }
        }

        // 1. Direct Player Object {id, name}
        if (obj.id && obj.name) {
            // Only update if we don't have it or it's new
            if (!this.players[obj.id]) {
                this.players[obj.id] = obj.name;
                console.log(`[Mapping] 🤝 Found: ${obj.name} (${obj.id})`);
            }
        }

        // 2. Players Dictionary {"ID": {name: "Bob"}}
        if (obj.players) {
            for (const [id, p] of Object.entries(obj.players)) {
                if (p.name) {
                    this.players[id] = p.name;
                    // console.log(`[Mapping] 🤝 Found: ${p.name}`); // Optional noisy log
                }
            }
        }

        // Recurse (Deep Search) - pass visited set to prevent duplicate processing
        Object.values(obj).forEach(child => this.scanForNames(child, visited));
    }

    // Process game seats array to update seat mappings robustly
    processGameSeats(data) {
        if (!data.seats || !Array.isArray(data.seats)) {
            return;
        }
        
        // Iterate through seats array: [[seatIndex, playerID], ...]
        for (const [seatIndex, playerID] of data.seats) {
            if (playerID && seatIndex >= 0 && seatIndex < 10) {
                // Store server's index directly (this is the authoritative source)
                this.playerIDsToSeats[playerID] = seatIndex;
                
                // Clear old player from this seat (if any)
                const oldPlayerID = this.indexToPlayer[seatIndex];
                if (oldPlayerID && oldPlayerID !== playerID) {
                    // Remove old player's seat mapping
                    delete this.seatMap[oldPlayerID];
                    delete this.playerIDsToSeats[oldPlayerID];
                }
                
                // Set new player in this seat
                this.indexToPlayer[seatIndex] = playerID;
                this.seatMap[playerID] = seatIndex;
            }
        }
    }

    getName(id) {
        const rawName = this.players[id] || id;
        
        // Use playerIDsToSeats as the authoritative source (from server's data.seats)
        // Fallback to seatMap for backward compatibility
        const serverIndex = this.playerIDsToSeats[id] !== undefined 
            ? this.playerIDsToSeats[id] 
            : this.seatMap[id];
        
        // Only add [S#] tag if:
        // 1. Server index exists
        // 2. The player is still in that seat (check reverse map)
        if (serverIndex !== undefined && this.indexToPlayer[serverIndex] === id) {
            // Use server's index directly (map 0-9 to 1-10 for human readability)
            const humanSeat = serverIndex + 1;
            return `[S${humanSeat}] ${rawName}`;
        }
        
        // If no seat found or player is no longer in that seat, return raw name
        return rawName;
    }

    // --- GAME LOGIC (Only runs on gC packets) ---
    processGameChange(packet) {
        const data = packet.data || {};
        
        // Ensure we have seat data before processing - trigger scan if needed
        this.scanForNames(data);
        
        // Process game seats array to ensure robust seat mapping
        // This is the authoritative source for seat assignments
        this.processGameSeats(data);

        // 1. CAPTURE CARDS (For ML)
        if (data.pC) {
            for (const [id, info] of Object.entries(data.pC)) {
                if (info.cards && info.cards.length > 0) {
                    const visible = info.cards.filter(c => c.showing && c.value).map(c => c.value);
                    if (visible.length > 0) {
                        const name = this.getName(id);
                        console.log(`[ML_DATA] 🎴 ${name} @ ${id} shows [${visible.join(', ')}]`);
                    }
                }
            }
        }

        // 2. Hand Reset
        if (data.nR || (data.gT && data.gT[0] === 1 && data.gT[1] === 0)) {
             this.emit("-- starting hand --");
             this.currentBets = {};
             this.lastProcessedBets = {};  // Clear last processed bets on reset
             // NOTE: Do NOT clear seatMap or indexToPlayer on reset - seats persist across hands
             return;
        }

        // 3. Folds
        if (data.pGS) {
            for (const [id, status] of Object.entries(data.pGS)) {
                if (status === 'fold') {
                    // Ensure seat ID is available before emitting
                    if (this.seatMap[id] === undefined) {
                        this.scanForNames(data);
                    }
                    const name = this.getName(id);
                    this.emit(`${name} @ ${id} folds`);
                }
            }
        }

        // 4. Bets (with deduplication)
        if (data.tB) {
            for (const [id, val] of Object.entries(data.tB)) {
                if (val === '<D>') {
                    this.currentBets[id] = 0;
                    this.lastProcessedBets[id] = 0;  // Track that we processed this
                    continue;
                }
                const amount = parseInt(val);
                if (isNaN(amount)) continue;

                const prev = this.currentBets[id] || 0;
                const lastEmitted = this.lastProcessedBets[id] || 0;
                
                // Only emit if amount increased AND we haven't already emitted this exact amount
                if (amount > prev && amount > lastEmitted) {
                    // Ensure seat ID is available before emitting
                    if (this.seatMap[id] === undefined) {
                        this.scanForNames(data);
                    }
                    const name = this.getName(id);
                    
                    if (prev === 0) {
                        this.emit(`${name} @ ${id} bets ${amount}`);
                    } else {
                        this.emit(`${name} @ ${id} calls ${amount}`);
                    }
                    
                    // Track that we emitted this amount
                    this.lastProcessedBets[id] = amount;
                }
                
                // Always update current bets (even if we didn't emit)
                this.currentBets[id] = amount;
            }
        }
    }

    async emit(text) {
        console.log(`[Raw] ⚡ ${text}`);
        try {
            await axios.post(RUST_URL, { action_string: text });
        } catch (e) {}
    }
}

(async () => {
    console.log("--- Braid Engine: Wiretap v5 (Deep Scan) ---");
    
    const browser = await puppeteer.launch({
        headless: false, 
        defaultViewport: null,
        args: ['--start-maximized']
    });

    const page = await browser.newPage();
    const client = await page.target().createCDPSession();
    await client.send('Network.enable');

    const engine = new GameEngine();

    client.on('Network.webSocketFrameReceived', ({ response }) => {
        try {
            const payload = response.payloadData;
            if (!payload.startsWith('42')) return;
            const jsonStr = payload.substring(2);
            const [event, data] = JSON.parse(jsonStr);

            // --- FIX: SCAN EVERY PACKET FOR NAMES ---
            // "registered", "rEM", "gC" all contain player data.
            engine.scanForNames(data);

            // --- GAME LOGIC ONLY ON gC ---
            if (event === 'gC') {
                engine.processGameChange({ data });
            }
        } catch (e) {}
    });

    console.log(`[Browser] Navigating to: ${TARGET_URL}`);
    await page.goto(TARGET_URL, { waitUntil: 'networkidle2' });
    
    console.log("[Browser] 🔄 Refreshing once to grab full roster...");
    await page.reload({ waitUntil: 'networkidle2' });
    
})();