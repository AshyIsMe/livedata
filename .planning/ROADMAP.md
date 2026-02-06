# Roadmap: livedata

## Overview

Builds on existing log collection infrastructure to add process monitoring, configurable storage, and zero-config deployment. Extends the single binary monitoring tool with new data collection capabilities while maintaining minimal resource overhead and instant usability.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Process Monitoring Core** - Add process collection and web interface
- [x] **Phase 2: Storage Enhancements** - Configurable retention and schema evolution
- [ ] **Phase 3: Zero-Config Improvements** - Auto-detection and sensible defaults

## Phase Details

### Phase 1: Process Monitoring Core
**Goal**: Users can monitor system processes through the web interface
**Depends on**: Existing log collection infrastructure (complete)
**Requirements**: PROCE-01, PROCE-02, PROCE-03
**Success Criteria** (what must be TRUE):
  1. User can view list of running processes with PID, name, CPU %, memory %, user, and runtime in the web interface
  2. User can search/filter processes using fuzzy search (fzf-style) in the web interface
  3. Process data is collected at user-configurable intervals via CLI flag or config file
**Plans**: 4 plans

Plans:
- [x] 01-01-PLAN.md — Backend process collection with sysinfo and API endpoint
- [x] 01-02-PLAN.md — CLI integration and ApplicationController wiring
- [x] 01-03-PLAN.md — Frontend process table with search and auto-refresh
- [x] 01-04-PLAN.md — End-to-end verification checkpoint

### Phase 2: Storage Enhancements
**Goal**: Users can control data retention and schema changes
**Depends on**: Phase 1
**Requirements**: STOR-02, STOR-03
**Success Criteria** (what must be TRUE):
  1. User can configure data retention period (e.g., 7 days, 30 days, custom) via CLI flag or config file
  2. Application automatically deletes expired data based on retention policy
  3. Storage layer handles schema evolution without breaking existing data or requiring manual migration
**Plans**: 4 plans

Plans:
- [x] 02-01-PLAN.md — Configuration and Schema Migration Foundation
- [x] 02-02-PLAN.md — Data Retention & Cleanup Implementation
- [x] 02-03-PLAN.md — Process Metrics Persistence & Storage Health API
- [x] 02-04-PLAN.md — UI Navigation & Storage Health Visibility

### Phase 3: Zero-Config Improvements
**Goal**: Application works out of the box with sensible defaults
**Depends on**: Phase 2
**Requirements**: DEPL-02
**Success Criteria** (what must be TRUE):
  1. Application runs immediately after binary execution with no manual configuration required
  2. Application auto-detects available system resources (journald presence, disk space, CPU cores)
  3. Application uses sensible defaults for collection intervals (5s processes, continuous logs), retention (30 days), and web port (3000)
**Plans**: TBD

Plans:
- [ ] 03-01: System capability detection module
- [ ] 03-02: Sensible default configuration values
- [ ] 03-03: Graceful degradation when resources are unavailable

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Process Monitoring Core | 4/4 | Complete | 2026-02-02 |
| 2. Storage Enhancements | 4/4 | Complete | 2026-02-06 |
| 3. Zero-Config Improvements | 0/TBD | Not started | - |
