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
fn test_simple_journal_polling() {
    println!("=== Simple Journal Polling Test ===");

    // Skip test if logger command is not available
    if !is_logger_available() {
        println!("Skipping test: logger command not available");
        return;
    }

    // Create a reader and seek to tail
    let mut reader = match JournalLogReader::new() {
        Ok(r) => r,
        Err(e) => {
            println!("Failed to create journal reader: {}", e);
            return;
        }
    };
    reader.seek_to_tail().unwrap();

    // Send a test message
    let test_message = "simple polling test message";
    if let Err(e) = send_test_message(test_message) {
        println!("Failed to send test message: {}", e);
        return;
    }

    // Give it time to be written to journal
    thread::sleep(Duration::from_millis(100));

    reader.previous_skip(100).unwrap();

    // Read some entries and look for our message
    println!("Searching for message: {}", test_message);
    let mut found = false;

    let mut i = 0;
    loop {
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
        i += 1;
    }

    assert!(found, "Test message not found in journal");
    println!("Test completed successfully!");
}
