use livedata::journal_reader::JournalLogReader;
use std::process::Command;
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
fn test_real_time_journal_detection() {
    println!("=== Real-Time Journal Detection Test ===");

    // Skip test if logger command is not available
    if !is_logger_available() {
        println!("Skipping test: logger command not available");
        return;
    }

    let test_message = "real time detection test message";

    // Create a journal reader and position it correctly
    let mut reader = match JournalLogReader::new() {
        Ok(r) => {
            println!("Successfully created journal reader");
            r
        }
        Err(e) => {
            println!("Failed to create journal reader: {}", e);
            return;
        }
    };

    // Seek to tail to only get new entries
    if let Err(e) = reader.seek_to_tail() {
        println!("Failed to seek to tail: {}", e);
        return;
    }

    println!("Seeked to tail, sending test message...");

    // Send test message
    if let Err(e) = send_test_message(test_message) {
        println!("Failed to send test message: {}", e);
        return;
    }

    println!("Sent test message: {}", test_message);

    // Poll for the new entry
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(8);
    let mut found = false;

    while start_time.elapsed() < timeout && !found {
        match reader.next_entry() {
            Ok(Some(entry)) => {
                if let Some(message) = entry.get_message() {
                    println!("Found message: {}", message);
                    if message.contains(test_message) {
                        println!("Found our test message!");
                        found = true;
                        break;
                    }
                }
            }
            Ok(None) => {
                println!("No entry available, waiting...");
                thread::sleep(Duration::from_millis(500));
            }
            Err(e) => {
                println!("Error reading entry: {}", e);
                break;
            }
        }
    }

    assert!(found, "Test message not found in journal");
    println!("Real-time detection test passed successfully!");
}

#[test]
fn test_journal_reader_works() {
    println!("=== Journal Reader Basic Functionality Test ===");

    // Just test that we can create a reader and read something
    let mut reader = match JournalLogReader::new() {
        Ok(r) => {
            println!("Successfully created journal reader");
            r
        }
        Err(e) => {
            println!("Failed to create journal reader: {}", e);
            return;
        }
    };

    // Try to read a few entries
    let mut entries_read = 0;
    for i in 0..5 {
        match reader.next_entry() {
            Ok(Some(entry)) => {
                entries_read += 1;
                if let Some(message) = entry.get_message() {
                    println!("Entry {}: {}", entries_read, message);
                } else {
                    println!("Entry {}: No MESSAGE field", entries_read);
                }
            }
            Ok(None) => {
                println!("No more entries after {} attempts", i + 1);
                break;
            }
            Err(e) => {
                println!("Error reading entry: {}", e);
                break;
            }
        }
    }

    println!("Read {} entries from journal", entries_read);
    assert!(
        entries_read > 0,
        "Should be able to read at least one entry"
    );
}
