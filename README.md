# livedata

Live Data at your fingertips.

- Single binary `livedata`
- Streaming live logs and metrics (cpu, memory, processes, etc) data from your machine, web apis, s3 buckets etc
- Data cached locally in parquet format
- Search interface inspired by splunk and fzf


## Usage:

Just run `livedata`.

You can now explore all logs and metrics from the current machine.

## Features:

- local machine ingest:
    - journald, windows event log, macos console?
- remote machine ingesting (scraping) via ssh


## Architecture:

- stream everything to parquet files
    - <datadir>/hostname/journald/2025/12/20251201.parquet
    - <datadir>/hostname/journald/2025/12/20251202.parquet
    - etc
    - down to 1minute or 30 second or 5 second parquet file chunks
    - background job to aggregate them into larger daily files once the clock ticks over
- dynamic table views over the parquet datasets with duckdb
- Splunk-like web interface to search and build charts and dashboards
- Possibly a desktop gui?
