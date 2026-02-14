use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
fn test_duckdb_cli_opens_after_graceful_shutdown() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&data_dir).expect("create data dir");

    let bin_path = env!("CARGO_BIN_EXE_livedata");

    let mut child = Command::new(bin_path)
        .arg("--data-dir")
        .arg(&data_dir)
        .arg("--follow")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn livedata");

    // Give the app a moment to initialize and create the DB.
    thread::sleep(Duration::from_millis(750));

    let pid = child.id();
    let kill_status = Command::new("kill")
        .arg("-SIGINT")
        .arg(pid.to_string())
        .status()
        .expect("send SIGINT");
    assert!(kill_status.success(), "failed to send SIGINT to livedata");

    // Wait up to 5 seconds for graceful shutdown.
    let mut exit_status = None;
    for _ in 0..50 {
        if let Ok(Some(status)) = child.try_wait() {
            exit_status = Some(status);
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    if exit_status.is_none() {
        let _ = Command::new("kill")
            .arg("-SIGKILL")
            .arg(pid.to_string())
            .status();
        panic!("livedata did not exit after SIGINT");
    }

    let status = exit_status.unwrap();
    assert!(
        status.success(),
        "livedata exited with non-zero status: {}",
        status
    );

    let db_path = data_dir.join("livedata.duckdb");
    assert!(db_path.exists(), "database file was not created");

    let duckdb_status = Command::new("duckdb")
        .arg(&db_path)
        .arg("PRAGMA database_size;")
        .status()
        .expect("run duckdb cli");

    assert!(
        duckdb_status.success(),
        "duckdb CLI failed to open the database"
    );
}
