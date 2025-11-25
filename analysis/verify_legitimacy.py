#!/usr/bin/env python3
"""
Braid Engine Legitimacy Verification

Performs scientific-control style tests to verify that the engine is deterministic
and semantically correct. Requires the Rust server to be running locally on:
    http://127.0.0.1:3030
"""

import json
import time
from typing import List, Tuple

import requests

SERVER_URL = "http://127.0.0.1:3030/action"

GREEN = "\033[92m"
RED = "\033[91m"
CYAN = "\033[96m"
RESET = "\033[0m"


def send_action(action: str) -> dict:
    """Send an action string to the server and return the JSON response."""
    payload = {"action_string": action}
    response = requests.post(SERVER_URL, json=payload, timeout=5)
    response.raise_for_status()
    return response.json()


def reset_hand(tag: str = "verification") -> dict:
    """Send a hand reset delimiter."""
    return send_action(f"-- starting hand #{tag} --")


def pretty_result(name: str, passed: bool, details: str) -> None:
    color = GREEN if passed else RED
    icon = "✅ PASSED" if passed else "❌ FAILED"
    print(f"{color}{icon}{RESET} {CYAN}{name}{RESET}")
    print(details)
    print("-" * 60)


def determinism_check() -> Tuple[bool, str]:
    sequence = [
        "Seat 1 bets 10",
        "Seat 2 calls 10",
        "Seat 3 raises 20",
        "Seat 1 calls 20",
        "Seat 2 folds",
    ]

    traces: List[List[float]] = []
    for run in range(2):
        reset_hand(f"determinism_{run+1}")
        run_trace = []
        for action in sequence:
            resp = send_action(action)
            run_trace.append(resp["burau_trace_magnitude"])
        traces.append(run_trace)

    trace_a, trace_b = traces
    passed = trace_a == trace_b
    details = (
        f"Run A: {trace_a}\n"
        f"Run B: {trace_b}\n"
        f"Comparison: {'MATCH' if passed else 'MISMATCH'}"
    )
    return passed, details


def fold_semantics_check() -> Tuple[bool, str]:
    reset_hand("fold_semantics")
    prev_writhe = 0
    monotonic = True
    writhe_history = []
    for seat in [1, 2, 3]:
        resp = send_action(f"Seat {seat} folds")
        writhe = resp["writhe"]
        writhe_history.append(writhe)
        if writhe > prev_writhe:
            monotonic = False
        prev_writhe = writhe

    details = f"Writhe progression after folds: {writhe_history}"
    return monotonic, details


def reset_state_check() -> Tuple[bool, str]:
    reset_hand("reset_state")
    complex_actions = [
        "Seat 1 bets 50",
        "Seat 2 raises 100",
        "Seat 3 calls 100",
        "Seat 1 raises 200",
        "Seat 2 calls 200",
    ]
    last_response = None
    for action in complex_actions:
        last_response = send_action(action)

    if not last_response:
        return False, "No responses recorded for complex sequence."

    burau_before_reset = last_response["burau_trace_magnitude"]

    reset_resp = reset_hand("reset_check")

    writhe_after_reset = reset_resp["writhe"]
    burau_after_reset = reset_resp["burau_trace_magnitude"]

    passed = (
        burau_before_reset > 10.0
        and writhe_after_reset == 0
        and burau_after_reset >= 8.0
    )

    details = (
        f"Burau before reset: {burau_before_reset:.4f}\n"
        f"Writhe after reset: {writhe_after_reset}\n"
        f"Burau after reset: {burau_after_reset:.4f}"
    )
    return passed, details


def main():
    tests = [
        ("Determinism Check", determinism_check),
        ("Fold Semantics Check", fold_semantics_check),
        ("Reset State Check", reset_state_check),
    ]

    print(f"{CYAN}Running Braid Engine Legitimacy Verification...{RESET}\n")

    all_passed = True
    for name, func in tests:
        try:
            passed, details = func()
        except requests.RequestException as exc:
            passed = False
            details = f"HTTP error: {exc}\nIs the server running at {SERVER_URL}?"
        except Exception as exc:  # noqa: BLE001
            passed = False
            details = f"Unexpected error: {exc}"

        pretty_result(name, passed, details)
        all_passed = all_passed and passed
        time.sleep(0.5)

    if all_passed:
        print(f"{GREEN}All legitimacy tests passed!{RESET}")
    else:
        print(f"{RED}One or more tests failed. Investigate the logs above.{RESET}")


if __name__ == "__main__":
    main()




