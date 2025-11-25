use braid_engine::{expand_action, Action, ActionType, FingerprintState, IncrementalUpdate};
use csv::ReaderBuilder;
use poker_parser::{parse_record, pokernow, SeatResolver};
use std::fs::File;
use std::io::BufReader;

/// JSON output structure for each step
#[derive(serde::Serialize)]
struct StepOutput {
    step: usize,
    action: String,
    writhe: i32,
    burau_trace_magnitude: f64,
}

/// Runs the CLI mode
pub fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} [--format pokernow] [--reset-on-fold] <csv_file_path>", args[0]);
        std::process::exit(1);
    }

    // Check for flags
    let mut format_pokernow = false;
    let mut reset_on_fold = false;
    let mut csv_path = None;
    
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--format" && i + 1 < args.len() {
            if args[i + 1] == "pokernow" {
                format_pokernow = true;
            }
            i += 2;
        } else if args[i] == "--reset-on-fold" {
            reset_on_fold = true;
            i += 1;
        } else if csv_path.is_none() {
            csv_path = Some(&args[i]);
            i += 1;
        } else {
            i += 1;
        }
    }

    let csv_path = csv_path.ok_or("Missing CSV file path")?;

    // Open the CSV file
    let file = File::open(csv_path)?;
    let reader = BufReader::new(file);

    // Initialize components
    let mut seat_resolver = SeatResolver::new();
    let mut fingerprint = FingerprintState::new(12); // Use 12 to handle player churn safely (modulo problem gave me absolute hell)
    let mut current_seat = None;
    let mut step = 0;

    if format_pokernow {
        // Process PokerNow format
        let mut csv_reader = ReaderBuilder::new()
            .has_headers(true)
            .from_reader(reader);

        // Deserialize into PokerNowRow
        for result in csv_reader.deserialize() {
            let row: pokernow::PokerNowRow = result?;
            
            // Parse the row to extract action
            if let Some((player_id, action_type, amount)) = pokernow::parse_row(&row) {
                // Resolve player_id to Seat
                let seat = seat_resolver.get_or_assign_seat(&player_id);
                
                // Create Action
                let action = Action::new(seat, action_type, amount);
                
                // Process the action (same logic as generic parser)
                process_action(
                    action,
                    &mut fingerprint,
                    &mut current_seat,
                    &mut step,
                    reset_on_fold,
                )?;
            }
            // If parse_row returns None, skip this row (filtered out)
        }
    } else {
        // Process generic format
        let mut csv_reader = ReaderBuilder::new()
            .has_headers(true)
            .from_reader(reader);

        // Process each record
        for result in csv_reader.records() {
            let record = result?;
            
            // Parse the action
            let action = parse_record(&record, &mut seat_resolver)?;
            
            // Process the action
            process_action(
                action,
                &mut fingerprint,
                &mut current_seat,
                &mut step,
                reset_on_fold,
            )?;
        }
    }

    Ok(())
}

/// Processes an action and updates the fingerprint state.
fn process_action(
    action: Action,
    fingerprint: &mut FingerprintState,
    current_seat: &mut Option<braid_engine::Seat>,
    step: &mut usize,
    reset_on_fold: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Handle Reset action (hand delimiter detected)
    if action.action_type == ActionType::Reset {
        fingerprint.reset();
        *current_seat = None;
        *step = 0;
        println!("--- HAND RESET ---");
        return Ok(());
    }
    
    // Reset on fold if flag is set (heuristic for end of hand)
    if reset_on_fold && action.action_type == ActionType::Fold {
        fingerprint.reset();
        *current_seat = None;
        // Don't increment step, as this is a reset marker
        // We'll still output the fold action, but with reset state
    }
    
    // Expand the action to generators
    // If this is the first action, we start from the action's seat
    // Otherwise, we move from the previous seat to the current action's seat
    let from_seat = current_seat.unwrap_or(action.seat);
    let generators = expand_action(from_seat, action.seat, fingerprint.dimension());
    
    // Update current seat
    *current_seat = Some(action.seat);

    // Process each generator
    for gen in &generators {
        fingerprint.update(gen);
    }

    *step += 1;

    // Format action description
    let action_desc = format!(
        "Seat {} {} (${})",
        action.seat.value(),
        format_action_type(action.action_type),
        action.amount
    );

    // Calculate Burau trace magnitude
    let trace_magnitude = fingerprint.burau_trace_magnitude();

    // Output JSON line
    let output = StepOutput {
        step: *step,
        action: action_desc,
        writhe: fingerprint.writhe,
        burau_trace_magnitude: trace_magnitude,
    };

    println!("{}", serde_json::to_string(&output)?);
    
    Ok(())
}

/// Formats an ActionType as a string for display.
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

