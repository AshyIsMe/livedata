# livedata

Live Data at your fingertips.

- Single binary `livedata`
- Streaming live logs and metrics (cpu, memory, processes, etc) data from your machine, web apis, s3 buckets etc
- Data cached locally in duckdb format
- Search interface inspired by splunk and fzf


## Usage:

Just run `livedata`.

You can now explore all logs and metrics from the current machine.

## Features:

- local machine ingest:
    - journald, windows event log, macos console?
- remote machine ingesting (scraping) via ssh


## Architecture:

- stream everything to duckdb (later can do parquet on object storage)
- Splunk-like web interface to search and build charts and dashboards
- Possibly a desktop gui?
