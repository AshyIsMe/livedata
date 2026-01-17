#!/bin/bash

# Journal Integration Test Runner
# This script runs the journal listener tests that verify integration with the logger command

set -e

echo "=== Journal Integration Test Runner ==="
echo

# Check if logger command is available
if ! command -v logger &> /dev/null; then
    echo "ERROR: 'logger' command is not available on this system"
    echo "This test requires the logger command to send messages to journald"
    exit 1
fi

# Check if we're running on a system with systemd/journald
if ! systemctl --version &> /dev/null; then
    echo "WARNING: systemd doesn't appear to be available"
    echo "This test is designed to work with systemd journald"
fi

echo "✓ Prerequisites check passed"
echo

# Build the project
echo "Building the project..."
cargo build --release
echo "✓ Build completed"
echo

# Run the journal integration tests
echo "Running journal integration tests..."
echo

# Test 1: Basic logger message detection
echo "--- Test 1: Basic logger message detection ---"
timeout 30s cargo test --release test_journal_listens_for_logger_messages -- --nocapture || {
    echo "✗ Test 1 failed or timed out"
    exit 1
}
echo "✓ Test 1 passed"
echo

# Test 2: Multiple logger messages
echo "--- Test 2: Multiple logger messages ---"
timeout 45s cargo test --release test_multiple_logger_messages -- --nocapture || {
    echo "✗ Test 2 failed or timed out"
    exit 1
}
echo "✓ Test 2 passed"
echo

# Test 3: Real-time journal listening
echo "--- Test 3: Real-time journal listening ---"
timeout 30s cargo test --release test_journal_reader_real_time -- --nocapture || {
    echo "✗ Test 3 failed or timed out"
    exit 1
}
echo "✓ Test 3 passed"
echo

echo "=== All journal integration tests passed! ==="
echo
echo "The journal listener successfully:"
echo "- Detects messages sent via the 'logger' command"
echo "- Captures multiple sequential messages"
echo "- Works in real-time to detect new journal entries"
echo "- Properly extracts and structures log entry data"