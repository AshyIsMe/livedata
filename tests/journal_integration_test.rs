use livedata::journal_reader::JournalLogReader;
use livedata::log_entry::LogEntry;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Helper function to check if logger command is available
fn is_logger_available() -> bool {
    Command::new("which")
        .arg("logger")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Helper function to send a test message to the journal
fn send_test_message(message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("logger").arg(message).output()?;

    if !output.status.success() {
        return Err(format!("Logger command failed: {:?}", output).into());
    }

    Ok(())
}

#[test]
fn test_journal_listens_for_logger_messages() {
    // Skip test if logger command is not available
    if !is_logger_available() {
        println!("Skipping test: logger command not available");
        return;
    }

    // Test that journal listener picks up messages from logger command
    let test_message = "integration test message from logger";
    let captured_entries: Arc<Mutex<Vec<LogEntry>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured_entries);
    let test_msg = test_message.to_string();

    // Start journal reader in a separate thread
    let reader_handle = thread::spawn(move || {
        let mut reader = match JournalLogReader::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to create journal reader: {}", e);
                return;
            }
        };

        // Seek to tail to only get new entries
        if let Err(e) = reader.seek_to_tail() {
            eprintln!("Failed to seek to tail: {}", e);
            return;
        }

        println!("Journal reader started, waiting for message: {}", test_msg);

        // Listen for entries for a limited time - using simple polling approach
        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(10);
        let mut entries_checked = 0;

        while start_time.elapsed() < timeout {
            // Try to read the entry directly without waiting
            if let Ok(Some(entry)) = reader.next_entry() {
                entries_checked += 1;
                println!(
                    "Checking entry #{}: {:?}",
                    entries_checked,
                    entry.get_message()
                );

                if let Some(message) = entry.get_message() {
                    if message.contains(&test_msg) {
                        println!("Found matching message: {}", message);
                        let mut entries = captured_clone.lock().unwrap();
                        entries.push(entry.clone());
                        break;
                    }
                }
            } else {
                // No entry available, sleep briefly
                thread::sleep(Duration::from_millis(200));
            }
        }

        println!(
            "Reader thread finished, checked {} entries",
            entries_checked
        );
    });

    // Give the reader a moment to start
    thread::sleep(Duration::from_millis(500));

    // Send test message using helper function
    if let Err(e) = send_test_message(test_message) {
        eprintln!("Failed to send test message: {}", e);
        return;
    }

    println!("Successfully sent test message to journal");

    // Wait for the reader thread to complete
    reader_handle.join().unwrap();

    // Verify we captured the test message
    let entries = captured_entries.lock().unwrap();
    assert!(!entries.is_empty(), "No entries captured from journal");

    // Verify the captured entry contains our test message
    let captured_entry = &entries[0];
    assert_eq!(
        captured_entry.get_message().map(|s| s.as_str()),
        Some(test_message)
    );

    // Verify the entry has expected fields
    assert!(captured_entry.get_field("__REALTIME_TIMESTAMP").is_some());
    assert!(captured_entry.get_field("_PID").is_some());

    println!(
        "Successfully captured journal entry with message: {}",
        test_message
    );
}

#[test]
fn test_simple_journal_polling() {
    println!("=== Simple Journal Polling Test ===");

    // Skip test if logger command is not available
    if !is_logger_available() {
        println!("Skipping test: logger command not available");
        return;
    }

    // Send a test message first
    let test_message = "simple polling test message";
    if let Err(e) = send_test_message(test_message) {
        println!("Failed to send test message: {}", e);
        return;
    }

    // Give it time to be written to journal
    thread::sleep(Duration::from_millis(1000));

    // Create a reader and don't seek to tail - read from current position
    let mut reader = match JournalLogReader::new() {
        Ok(r) => r,
        Err(e) => {
            println!("Failed to create journal reader: {}", e);
            return;
        }
    };

    // Read some entries and look for our message
    println!("Searching for message: {}", test_message);
    let mut found = false;

    for i in 0..50 {
        // Check up to 50 entries
        match reader.next_entry() {
            Ok(Some(entry)) => {
                if let Some(message) = entry.get_message() {
                    println!("Entry {}: {}", i + 1, message);
                    if message.contains(test_message) {
                        println!("Found our test message!");
                        found = true;
                        break;
                    }
                }
            }
            Ok(None) => {
                println!("No more entries after {} checks", i + 1);
                break;
            }
            Err(e) => {
                println!("Error reading entry: {}", e);
                break;
            }
        }
    }

    assert!(found, "Test message not found in journal");
    println!("Test completed successfully!");
}
