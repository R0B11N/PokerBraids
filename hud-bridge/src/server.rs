use braid_engine::{expand_action, Action, ActionType, FingerprintState, Seat};
use futures::{SinkExt, StreamExt};
use poker_parser::{pokernow, SeatResolver};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{broadcast, RwLock};
use warp::Filter;

/// Shared state for the server
pub type SharedState = Arc<RwLock<ServerState>>;

/// Server state containing fingerprint and session info
#[derive(Clone)]
pub struct ServerState {
    pub fingerprint: FingerprintState,
    pub seat_resolver: SeatResolver,
    pub current_seat: Option<Seat>,
    pub step: usize,
    pub reset_on_fold: bool,
}

impl ServerState {
    pub fn new(reset_on_fold: bool) -> Self {
        // Use dimension 12 to provide buffer for player churn
        // Even on 9-handed tables, this reduces hash collisions before modulo mapping kicks in, as I've found out the hard way xd
        ServerState {
            fingerprint: FingerprintState::new(12),
            seat_resolver: SeatResolver::new(),
            current_seat: None,
            step: 0,
            reset_on_fold,
        }
    }
}

/// JSON request for POST /action
#[derive(serde::Deserialize)]
pub struct ActionRequest {
    pub action_string: String,
}

/// JSON response for fingerprint updates
#[derive(serde::Serialize, Clone)]
pub struct FingerprintResponse {
    pub step: usize,
    pub action: String,
    #[serde(rename = "global")]
    pub global_metrics: GlobalMetrics,
    #[serde(rename = "players")]
    pub player_metrics: std::collections::HashMap<String, PlayerMetrics>,
}

/// Global topological metrics
#[derive(serde::Serialize, Clone)]
pub struct GlobalMetrics {
    pub writhe: i32,
    pub burau: f64,
}

/// Player-specific metrics (simplified for JSON)
#[derive(serde::Serialize, Clone)]
pub struct PlayerMetrics {
    pub name: String,
    pub writhe: i32,
    pub complexity: f64,
}

/// Processes an action and updates the shared state
pub fn process_action(
    action: Action,
    state: &mut ServerState,
) -> Result<FingerprintResponse, Box<dyn std::error::Error>> {
    // Handle Reset action (hand delimiter detected)
    if action.action_type == ActionType::Reset {
        state.fingerprint.reset();
        state.current_seat = None;
        state.step = 0; // Reset step counter
        
        println!("--- HAND RESET ---");
        
        return Ok(FingerprintResponse {
            step: 0,
            action: "--- HAND RESET ---".to_string(),
            global_metrics: GlobalMetrics {
                writhe: 0,
                burau: state.fingerprint.burau_trace_magnitude(),
            },
            player_metrics: HashMap::new(),
        });
    }
    
    // Reset on fold if flag is set
    if state.reset_on_fold && action.action_type == ActionType::Fold {
        state.fingerprint.reset();
        state.current_seat = None;
    }

    // Expand the action to generators
    let from_seat = state.current_seat.unwrap_or(action.seat);
    let generators = expand_action(from_seat, action.seat, state.fingerprint.dimension());

    // Get player name for this seat
    let player_name = state.seat_resolver.get_player_name(action.seat);

    // Update current seat
    state.current_seat = Some(action.seat);

    // Process each generator with per-seat tracking
    for gen in &generators {
        state.fingerprint.update_for_seat(gen, action.seat.value(), player_name.clone());
    }

    state.step += 1;

    // Format action description
    let action_desc = format!(
        "Seat {} {} (${})",
        action.seat.value(),
        format_action_type(action.action_type),
        action.amount
    );

    // Calculate Burau trace magnitude
    let trace_magnitude = state.fingerprint.burau_trace_magnitude();

    // Build player metrics map
    let mut player_metrics_map = HashMap::new();
    for (seat_num, metrics) in &state.fingerprint.player_stats {
        player_metrics_map.insert(
            seat_num.to_string(),
            PlayerMetrics {
                name: metrics.name.clone(),
                writhe: metrics.writhe,
                complexity: metrics.complexity,
            },
        );
    }

    Ok(FingerprintResponse {
        step: state.step,
        action: action_desc,
        global_metrics: GlobalMetrics {
            writhe: state.fingerprint.writhe,
            burau: trace_magnitude,
        },
        player_metrics: player_metrics_map,
    })
}

/// Formats an ActionType as a string for display
fn format_action_type(action_type: ActionType) -> &'static str {
    match action_type {
        ActionType::Fold => "fold",
        ActionType::Check => "check",
        ActionType::Call => "call",
        ActionType::Bet => "bet",
        ActionType::Raise => "raise",
        ActionType::ReRaise => "reraise",
        ActionType::AllIn => "allin",
        ActionType::Reset => "reset",
    }
}

/// Parses an action string into an Action
pub fn parse_action_string(
    action_string: &str,
    state: &mut ServerState,
) -> Result<Action, Box<dyn std::error::Error>> {
    // Try to parse as PokerNow format first
    // Create a dummy PokerNowRow for parsing
    let row = pokernow::PokerNowRow {
        entry: action_string.to_string(),
        at: String::new(),
        order: 0,
    };

    if let Some((player_id, action_type, amount)) = pokernow::parse_row(&row) {
        let seat = state.seat_resolver.get_or_assign_seat(&player_id);
        Ok(Action::new(seat, action_type, amount))
    } else {
        Err("Failed to parse action string".into())
    }
}

/// POST /action endpoint handler
pub async fn handle_action(
    req: ActionRequest,
    state: SharedState,
    tx: broadcast::Sender<FingerprintResponse>,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Parse the action
    let mut state_guard = state.write().await;
    let action = match parse_action_string(&req.action_string, &mut *state_guard) {
        Ok(a) => a,
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                warp::http::StatusCode::BAD_REQUEST,
            ));
        }
    };

    // Process the action
    let response = match process_action(action, &mut *state_guard) {
        Ok(r) => r,
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    // Broadcast to WebSocket clients
    let _ = tx.send(response.clone());

    // Return the response
    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        warp::http::StatusCode::OK,
    ))
}

/// WebSocket connection handler
pub async fn handle_ws(
    ws: warp::ws::WebSocket,
    tx: broadcast::Sender<FingerprintResponse>,
) {
    let (mut ws_tx, _ws_rx) = ws.split();
    let mut rx = tx.subscribe();

    // Send initial state
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let json = match serde_json::to_string(&msg) {
                Ok(j) => j,
                Err(_) => continue,
            };
            if ws_tx.send(warp::ws::Message::text(json)).await.is_err() {
                break;
            }
        }
    });
}

/// Creates the server routes
pub fn create_routes(
    state: SharedState,
    tx: broadcast::Sender<FingerprintResponse>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let state_filter = warp::any().map(move || state.clone());
    let tx_filter = warp::any().map(move || tx.clone());

    // POST /action
    let action_route = warp::path("action")
        .and(warp::post())
        .and(warp::body::json())
        .and(state_filter.clone())
        .and(tx_filter.clone())
        .and_then(handle_action);

    // GET /ws
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(tx_filter)
        .map(|ws: warp::ws::Ws, tx| {
            ws.on_upgrade(move |socket| handle_ws(socket, tx))
        });

    // CORS headers
    // Note: allow_any_origin() is used for development
    // In production, restrict to: .allow_origin("https://www.pokernow.club")
    let cors = warp::cors()
        .allow_any_origin()  // Allows requests from pokernow.club and other origins
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "OPTIONS"])
        .allow_credentials(false);  // Set to true if cookies/auth needed

    action_route.or(ws_route).with(cors)
}

/// Starts the web server
pub async fn start_server(reset_on_fold: bool) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize shared state
    let state: SharedState = Arc::new(RwLock::new(ServerState::new(reset_on_fold)));
    
    // Create broadcast channel for WebSocket clients
    let (tx, _rx) = broadcast::channel::<FingerprintResponse>(100);
    
    // Create routes
    let routes = create_routes(state, tx);
    
    // Start server
    let addr = ([127, 0, 0, 1], 3030);
    println!("Server starting on http://127.0.0.1:3030/");
    println!("Endpoints:");
    println!("  POST http://127.0.0.1:3030/action");
    println!("  GET  ws://127.0.0.1:3030/ws");
    
    warp::serve(routes).run(addr).await;
    
    Ok(())
}

