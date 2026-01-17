# Journal Integration Test

This test verifies that the journal listener properly picks up log entries sent via the `logger` command.

## Test Overview

The integration test consists of three test cases:

1. **Basic Logger Message Detection** (`test_journal_listens_for_logger_messages`)
   - Starts the journal listener
   - Sends a single test message via `logger`
   - Verifies the message is captured and parsed correctly

2. **Multiple Logger Messages** (`test_multiple_logger_messages`)
   - Starts the journal listener
   - Sends multiple test messages via `logger`
   - Verifies all messages are captured in sequence

3. **Real-time Journal Listening** (`test_journal_reader_real_time`)
   - Starts the journal listener and seeks to tail
   - Sends a test message with unique timestamp
   - Verifies real-time detection of new journal entries

## Running the Tests

### Quick Run
```bash
# Run all journal integration tests
./test_journal_integration.sh
```

### Manual Test Execution
```bash
# Build the project first
cargo build --release

# Run individual tests
cargo test --release test_journal_listens_for_logger_messages -- --nocapture
cargo test --release test_multiple_logger_messages -- --nocapture  
cargo test --release test_journal_reader_real_time -- --nocapture

# Run all integration tests
cargo test --release journal_integration -- --nocapture
```

## Test Requirements

- Linux system with systemd/journald
- `logger` command available (standard on most Linux distributions)
- Sufficient permissions to read from the journal

## What the Tests Verify

1. **Journal Connection**: The journal reader can successfully connect to systemd-journald
2. **Message Detection**: Messages sent via `logger` appear in the journal stream
3. **Message Parsing**: Journal entries are correctly parsed into `LogEntry` structures
4. **Field Extraction**: Standard fields like `MESSAGE`, `_PID`, `__REALTIME_TIMESTAMP` are extracted
5. **Real-time Processing**: The listener can detect new messages as they are written
6. **Concurrency**: The reader operates in a separate thread without blocking

## Troubleshooting

### Test Fails with "No entries captured"
- Check if systemd is running: `systemctl --version`
- Verify journal permissions: Try running with `sudo`
- Check if messages are being written: `journalctl -f` and send a test message

### Test Times Out
- The timeout is generous (10-15 seconds), but system load can affect this
- Try running with fewer system processes running
- Check if journal is accessible: `journalctl -n 10`

### Logger Command Not Found
- Install util-linux package: `sudo apt-get install util-linux` (Ubuntu/Debian)
- Or `sudo yum install util-linux` (RHEL/CentOS)

## Expected Output

When successful, each test should output:
```
Successfully sent test message to journal
Successfully captured journal entry with message: integration test message from logger
```

And the final result should be:
```
=== All journal integration tests passed! ===
```

The tests verify that your journal listener implementation is working correctly and can integrate with the system's logging infrastructure.