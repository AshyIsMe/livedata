use livedata::journal_reader::JournalLogReader;
use std::process::Command;
use std::thread;
use std::time::Duration;
use systemd::journal::{Journal, OpenOptions};

#[test]
fn test_direct_journal_access() {
    println!("=== Direct Journal Access Test ===");

    // Send a test message first
    let test_message = "direct access test message";
    if let Ok(_) = Command::new("logger").arg(test_message).output() {
        println!("Sent test message: {}", test_message);
    }

    thread::sleep(Duration::from_millis(500));

    // Try to open journal directly with different options
    let mut journal = match OpenOptions::default()
        .system(true)
        .current_user(true)
        .local_only(false)
        .runtime_only(false)
        .open()
    {
        Ok(j) => {
            println!("Successfully opened journal");
            j
        }
        Err(e) => {
            println!("Failed to open journal: {}", e);
            return;
        }
    };

    // Try to read some entries without seeking
    println!("Reading entries from journal...");
    let mut entries_read = 0;

    for i in 0..10 {
        match journal.next_entry() {
            Ok(Some(entry)) => {
                entries_read += 1;
                if let Some(message) = entry.get("MESSAGE") {
                    println!("Entry {}: {}", entries_read, message);
                    if message.contains(test_message) {
                        println!("Found our test message!");
                        break;
                    }
                } else {
                    println!("Entry {}: No MESSAGE field", entries_read);
                }
            }
            Ok(None) => {
                println!("Entry {}: No more entries", entries_read + 1);
                break;
            }
            Err(e) => {
                println!("Entry {}: Error: {}", entries_read + 1, e);
                break;
            }
        }
    }

    println!("Total entries read: {}", entries_read);
}

#[test]
fn test_seek_and_wait() {
    println!("=== Seek and Wait Test ===");

    let test_message = "seek and wait test message";

    // Create journal reader without seeking first
    let mut journal = match OpenOptions::default()
        .system(true)
        .current_user(true)
        .local_only(false)
        .runtime_only(false)
        .open()
    {
        Ok(j) => {
            println!("Successfully opened journal");
            j
        }
        Err(e) => {
            println!("Failed to open journal: {}", e);
            return;
        }
    };

    // Seek to tail
    if let Err(e) = journal.seek_tail() {
        println!("Failed to seek to tail: {}", e);
        return;
    }

    println!("Seeked to tail, sending test message...");

    // Send test message
    if let Ok(_) = Command::new("logger").arg(test_message).output() {
        println!("Sent test message: {}", test_message);
    }

    // Wait a moment and then try to read
    println!("Waiting for message to appear...");
    thread::sleep(Duration::from_millis(100));

    journal.previous_skip(100).unwrap();

    // Try to read entries
    let mut found = false;
    for i in 0..100 {
        match journal.next_entry() {
            Ok(Some(entry)) => {
                if let Some(message) = entry.get("MESSAGE") {
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

    println!("Test completed, found message: {}", found);
}
