use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static SQL_TRACE_WRITER: OnceLock<Mutex<BufWriter<std::fs::File>>> = OnceLock::new();

pub fn init_sql_trace(path: &Path) -> std::io::Result<()> {
    if SQL_TRACE_WRITER.get().is_some() {
        return Ok(());
    }

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    let writer = BufWriter::new(file);

    let _ = SQL_TRACE_WRITER.set(Mutex::new(writer));
    Ok(())
}

pub fn trace_sql(sql: &str) {
    let writer = match SQL_TRACE_WRITER.get() {
        Some(writer) => writer,
        None => return,
    };

    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return;
    }

    if let Ok(mut writer) = writer.lock() {
        if trimmed.ends_with(';') {
            let _ = writeln!(writer, "{}", trimmed);
        } else {
            let _ = writeln!(writer, "{};", trimmed);
        }
        let _ = writer.flush();
    }
}
