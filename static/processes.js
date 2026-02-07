// Process monitoring table with Tabulator, search, and auto-refresh
// This file provides the interactive frontend for viewing system processes

let table;
let allProcesses = [];
let debounceTimer;
const DEBOUNCE_MS = 300;
let autoRefreshInterval;
let Tabulator;

async function loadTabulator() {
    const sources = [
        "https://unpkg.com/tabulator-tables@6.3.1/dist/js/tabulator_esm.min.js",
        "https://cdn.jsdelivr.net/npm/tabulator-tables@6.3.1/dist/js/tabulator_esm.min.js",
    ];

    let lastError;
    for (const src of sources) {
        try {
            const mod = await import(src);
            if (mod && mod.TabulatorFull) {
                return mod.TabulatorFull;
            }
            if (mod && mod.default) {
                return mod.default;
            }
        } catch (error) {
            lastError = error;
        }
    }

    throw lastError || new Error("Unable to load Tabulator module");
}

// Initialize on DOMContentLoaded
document.addEventListener('DOMContentLoaded', async () => {
    console.log('[ProcessMonitor] DOM loaded, starting initialization...');
    
    try {
        Tabulator = await loadTabulator();
    } catch (error) {
        console.error('[ProcessMonitor] Tabulator library not loaded!', error);
        showError('Failed to load Tabulator table library. Please refresh the page.');
        return;
    }
    
    console.log('[ProcessMonitor] Tabulator version:', Tabulator.prototype.version);
    
    // Check if table container exists
    const tableContainer = document.getElementById('processes-table');
    if (!tableContainer) {
        console.error('[ProcessMonitor] Table container #processes-table not found!');
        showError('Table container not found. Please refresh the page.');
        return;
    }
    
    try {
        initializeTable();
        initializeControls();
        fetchProcesses();
        console.log('[ProcessMonitor] Initialization complete');
    } catch (error) {
        console.error('[ProcessMonitor] Initialization failed:', error);
        showError('Failed to initialize process monitor: ' + error.message);
    }
});

/**
 * Initialize the Tabulator table with process columns
 */
function initializeTable() {
    console.log('[ProcessMonitor] Initializing Tabulator table...');
    
    table = new Tabulator("#processes-table", {
        layout: "fitColumns",
        initialSort: [{ column: "cpu_percent", dir: "desc" }],
        placeholder: "No processes found",
        data: [], // Start with empty data
        columns: [
            { 
                title: "PID", 
                field: "pid", 
                sorter: "number", 
                width: 100 
            },
            { 
                title: "Name", 
                field: "name", 
                sorter: "string",
                minWidth: 150
            },
            { 
                title: "CPU %", 
                field: "cpu_percent", 
                sorter: "number",
                formatter: (cell) => {
                    const val = cell.getValue();
                    return val !== null && val !== undefined ? val.toFixed(1) + "%" : "-";
                },
                width: 120,
                hozAlign: "right"
            },
            { 
                title: "Memory %", 
                field: "memory_percent", 
                sorter: "number",
                formatter: (cell) => {
                    const val = cell.getValue();
                    return val !== null && val !== undefined ? val.toFixed(1) + "%" : "-";
                },
                width: 120,
                hozAlign: "right"
            },
            { 
                title: "User", 
                field: "user_id", 
                sorter: "string",
                width: 150,
                formatter: (cell) => {
                    const val = cell.getValue();
                    if (!val) return "-";
                    // Extract numeric UID from "Uid(1234)" format
                    const match = String(val).match(/Uid\((\d+)\)/);
                    return match ? match[1] : val;
                }
            },
            { 
                title: "Runtime", 
                field: "runtime_display", 
                sorter: "string",
                width: 150
            }
        ]
    });
    
    console.log('[ProcessMonitor] Table initialized successfully');
}

/**
 * Set up event listeners for controls
 */
function initializeControls() {
    console.log('[ProcessMonitor] Initializing controls...');
    
    // Search input with debouncing
    const searchInput = document.getElementById('search-input');
    if (searchInput) {
        searchInput.addEventListener('input', (e) => {
            clearTimeout(debounceTimer);
            debounceTimer = setTimeout(() => {
                filterAndDisplay();
            }, DEBOUNCE_MS);
        });
    }
    
    // Manual refresh button
    const refreshButton = document.getElementById('refresh-button');
    if (refreshButton) {
        refreshButton.addEventListener('click', () => {
            console.log('[ProcessMonitor] Manual refresh triggered');
            fetchProcesses();
        });
    }
    
    // Auto-refresh toggle
    const autoRefreshCheckbox = document.getElementById('auto-refresh');
    if (autoRefreshCheckbox) {
        autoRefreshCheckbox.addEventListener('change', updateAutoRefresh);
    }
    
    // Refresh interval change
    const refreshInterval = document.getElementById('refresh-interval');
    if (refreshInterval) {
        refreshInterval.addEventListener('change', updateAutoRefresh);
    }
    
    // Start auto-refresh
    updateAutoRefresh();
    console.log('[ProcessMonitor] Controls initialized');
}

/**
 * Fetch process data from the API
 */
async function fetchProcesses() {
    console.log('[ProcessMonitor] Fetching process data...');
    
    try {
        const response = await fetch('/api/processes');
        
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        
        const data = await response.json();
        console.log(`[ProcessMonitor] Received ${data.processes ? data.processes.length : 0} processes`);
        
        if (!data.processes || !Array.isArray(data.processes)) {
            throw new Error('Invalid data format: processes array not found');
        }
        
        // Process data - calculate derived fields
        allProcesses = data.processes.map(proc => {
            const memoryPercent = calculateMemoryPercent(proc.memory_bytes);
            const runtimeDisplay = formatRuntime(proc.runtime_secs);
            
            return { 
                ...proc, 
                memory_percent: memoryPercent, 
                runtime_display: runtimeDisplay 
            };
        });
        
        console.log(`[ProcessMonitor] Processed ${allProcesses.length} processes for display`);
        
        // Update table with filtered data
        filterAndDisplay();
        
        // Update timestamp
        updateTimestamp(data.timestamp);
        
    } catch (error) {
        console.error('[ProcessMonitor] Failed to fetch processes:', error);
        showError('Failed to fetch process data: ' + error.message);
    }
}

/**
 * Calculate memory percentage from bytes
 * Assumes 16GB total memory (to be replaced with actual system memory API)
 */
function calculateMemoryPercent(memoryBytes) {
    if (!memoryBytes || memoryBytes === 0) return 0;
    const totalMemoryBytes = 16 * 1024 * 1024 * 1024; // 16GB in bytes
    return (memoryBytes / totalMemoryBytes) * 100;
}

/**
 * Format runtime seconds to human-readable string
 * Examples: "45s", "5m 30s", "2h 15m", "3d 12h"
 */
function formatRuntime(seconds) {
    if (!seconds || seconds < 0) return "-";
    
    if (seconds < 60) {
        return seconds + "s";
    }
    
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) {
        const remainingSecs = seconds % 60;
        return remainingSecs > 0 ? `${minutes}m ${remainingSecs}s` : `${minutes}m`;
    }
    
    const hours = Math.floor(minutes / 60);
    if (hours < 24) {
        const remainingMins = minutes % 60;
        return remainingMins > 0 ? `${hours}h ${remainingMins}m` : `${hours}h`;
    }
    
    const days = Math.floor(hours / 24);
    const remainingHours = hours % 24;
    return remainingHours > 0 ? `${days}d ${remainingHours}h` : `${days}d`;
}

/**
 * Filter processes based on search query and update table
 */
function filterAndDisplay() {
    if (!table) {
        console.error('[ProcessMonitor] Table not initialized in filterAndDisplay');
        return;
    }
    
    const query = document.getElementById('search-input').value.toLowerCase().trim();
    
    console.log(`[ProcessMonitor] Filtering with query: "${query}"`);
    
    if (!query) {
        console.log(`[ProcessMonitor] Setting ${allProcesses.length} processes (no filter)`);
        table.setData(allProcesses);
        return;
    }
    
    // Fuzzy-ish filter: all query chars must appear in order in searchable text
    const filtered = allProcesses.filter(proc => {
        const searchText = `${proc.pid} ${proc.name} ${proc.user_id || ''} ${proc.cpu_percent !== undefined ? proc.cpu_percent.toFixed(1) : ''}`.toLowerCase();
        return fuzzyMatch(query, searchText);
    });
    
    console.log(`[ProcessMonitor] Filtered to ${filtered.length} processes`);
    table.setData(filtered);
}

/**
 * Simple fuzzy matching: all query chars must appear in order in text
 */
function fuzzyMatch(query, text) {
    let textIndex = 0;
    
    for (let char of query) {
        textIndex = text.indexOf(char, textIndex);
        if (textIndex === -1) {
            return false;
        }
        textIndex++;
    }
    
    return true;
}

/**
 * Update auto-refresh interval based on user settings
 */
function updateAutoRefresh() {
    // Clear existing interval
    if (autoRefreshInterval) {
        clearInterval(autoRefreshInterval);
        autoRefreshInterval = null;
    }
    
    const enabled = document.getElementById('auto-refresh').checked;
    const intervalSecs = parseInt(document.getElementById('refresh-interval').value);
    
    if (enabled && intervalSecs > 0) {
        autoRefreshInterval = setInterval(fetchProcesses, intervalSecs * 1000);
        console.log(`[ProcessMonitor] Auto-refresh enabled: ${intervalSecs}s interval`);
    } else {
        console.log('[ProcessMonitor] Auto-refresh disabled');
    }
}

/**
 * Update the "last updated" timestamp display
 */
function updateTimestamp(isoTimestamp) {
    if (!isoTimestamp) return;
    
    const date = new Date(isoTimestamp);
    const secondsAgo = Math.floor((Date.now() - date.getTime()) / 1000);
    const timeString = date.toLocaleTimeString();
    
    const element = document.getElementById('last-updated');
    if (element) {
        element.textContent = `Updated ${secondsAgo}s ago (${timeString})`;
    }
}

/**
 * Display error message to user
 */
function showError(message) {
    console.error('[ProcessMonitor]', message);
    
    // Create or update error display
    let errorDiv = document.getElementById('process-error-message');
    if (!errorDiv) {
        errorDiv = document.createElement('div');
        errorDiv.id = 'process-error-message';
        errorDiv.style.cssText = 'background: #ffebee; color: #c62828; padding: 15px; margin: 20px 0; border-radius: 4px; border: 1px solid #ef9a9a;';
        
        const tableContainer = document.getElementById('processes-table');
        if (tableContainer && tableContainer.parentNode) {
            tableContainer.parentNode.insertBefore(errorDiv, tableContainer);
        }
    }
    
    errorDiv.textContent = message;
    errorDiv.style.display = 'block';
}
