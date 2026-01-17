use livedata::journal_reader::JournalLogReader;
use livedata::log_entry::LogEntry;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[test]
fn test_simple_journal_read() {
    println!("=== Simple Journal Read Test ===");

    // First, send a test message
    let test_message = "simple test message";
    if let Ok(_) = Command::new("logger").arg(test_message).output() {
        println!("Sent test message: {}", test_message);
    }

    // Give it a moment to be written
    thread::sleep(Duration::from_millis(1000));

    // Try to create a reader and read some entries
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

    // Try to read the last few entries
    println!("Reading the last 5 entries from journal...");
    for i in 0..5 {
        match reader.next_entry() {
            Ok(Some(entry)) => {
                println!("Entry {}: {:?}", i + 1, entry.get_message());
            }
            Ok(None) => {
                println!("Entry {}: No entry available", i + 1);
            }
            Err(e) => {
                println!("Entry {}: Error reading entry: {}", i + 1, e);
            }
        }
    }
}

#[test]
fn test_journal_from_head() {
    println!("=== Journal From Head Test ===");

    // Send a test message first
    let test_message = "head test message";
    if let Ok(_) = Command::new("logger").arg(test_message).output() {
        println!("Sent test message: {}", test_message);
    }

    thread::sleep(Duration::from_millis(500));

    // Create a reader but don't seek to tail
    let mut reader = match JournalLogReader::new() {
        Ok(r) => r,
        Err(e) => {
            println!("Failed to create journal reader: {}", e);
            return;
        }
    };

    // Instead of seeking to tail, let's go to head and read forward
    // This is a bit of a hack - we'll just keep calling next_entry to get to recent entries

    // Skip ahead by reading many entries until we find recent ones
    println!("Searching for recent entries...");
    let mut entries_checked = 0;
    let mut found_recent = false;

    while entries_checked < 1000 {
        // Limit to prevent infinite loop
        match reader.next_entry() {
            Ok(Some(entry)) => {
                entries_checked += 1;
                if let Some(message) = entry.get_message() {
                    if message.contains(test_message) {
                        println!("Found our test message: {}", message);
                        found_recent = true;
                        break;
                    }
                }

                // Check if this is a recent entry (within last minute)
                let now = chrono::Utc::now();
                let age = now - entry.timestamp;
                if age.num_seconds() < 60 {
                    println!("Found recent entry: {:?}", entry.get_message());
                    found_recent = true;
                    break;
                }
            }
            Ok(None) => {
                // No more entries
                break;
            }
            Err(e) => {
                println!("Error reading entry: {}", e);
                break;
            }
        }
    }

    println!(
        "Checked {} entries, found recent: {}",
        entries_checked, found_recent
    );
}
