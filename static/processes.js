// Process monitoring table with Tabulator, search, and auto-refresh
// This file provides the interactive frontend for viewing system processes

let table;
let allProcesses = [];
let debounceTimer;
const DEBOUNCE_MS = 300;
let autoRefreshInterval;

// Initialize on DOMContentLoaded
document.addEventListener('DOMContentLoaded', () => {
    initializeTable();
    initializeControls();
    fetchProcesses();
});

/**
 * Initialize the Tabulator table with process columns
 */
function initializeTable() {
    table = new Tabulator("#processes-table", {
        layout: "fitColumns",
        initialSort: [{ column: "cpu_percent", dir: "desc" }],
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
                formatter: (cell) => cell.getValue().toFixed(1) + "%",
                width: 120,
                hozAlign: "right"
            },
            { 
                title: "Memory %", 
                field: "memory_percent", 
                sorter: "number",
                formatter: (cell) => cell.getValue().toFixed(1) + "%",
                width: 120,
                hozAlign: "right"
            },
            { 
                title: "User", 
                field: "user_id", 
                sorter: "string",
                width: 150,
                formatter: (cell) => cell.getValue() || "-"
            },
            { 
                title: "Runtime", 
                field: "runtime_display", 
                sorter: "string",
                width: 150
            }
        ]
    });
}

/**
 * Set up event listeners for controls
 */
function initializeControls() {
    // Search input with debouncing
    document.getElementById('search-input').addEventListener('input', (e) => {
        clearTimeout(debounceTimer);
        debounceTimer = setTimeout(() => {
            filterAndDisplay();
        }, DEBOUNCE_MS);
    });
    
    // Manual refresh button
    document.getElementById('refresh-button').addEventListener('click', () => {
        fetchProcesses();
    });
    
    // Auto-refresh toggle
    document.getElementById('auto-refresh').addEventListener('change', updateAutoRefresh);
    
    // Refresh interval change
    document.getElementById('refresh-interval').addEventListener('change', updateAutoRefresh);
    
    // Start auto-refresh
    updateAutoRefresh();
}

/**
 * Fetch process data from the API
 */
async function fetchProcesses() {
    try {
        const response = await fetch('/api/processes');
        
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        
        const data = await response.json();
        
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
        
        // Update table with filtered data
        filterAndDisplay();
        
        // Update timestamp
        updateTimestamp(data.timestamp);
        
    } catch (error) {
        console.error('Failed to fetch processes:', error);
        showError('Failed to fetch process data. Please try again.');
    }
}

/**
 * Calculate memory percentage from bytes
 * Assumes 16GB total memory (to be replaced with actual system memory API)
 */
function calculateMemoryPercent(memoryBytes) {
    const totalMemoryBytes = 16 * 1024 * 1024 * 1024; // 16GB in bytes
    return (memoryBytes / totalMemoryBytes) * 100;
}

/**
 * Format runtime seconds to human-readable string
 * Examples: "45s", "5m 30s", "2h 15m", "3d 12h"
 */
function formatRuntime(seconds) {
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
    const query = document.getElementById('search-input').value.toLowerCase().trim();
    
    if (!query) {
        table.setData(allProcesses);
        return;
    }
    
    // Fuzzy-ish filter: all query chars must appear in order in searchable text
    const filtered = allProcesses.filter(proc => {
        const searchText = `${proc.pid} ${proc.name} ${proc.user_id || ''} ${proc.cpu_percent.toFixed(1)}`.toLowerCase();
        return fuzzyMatch(query, searchText);
    });
    
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
    }
}

/**
 * Update the "last updated" timestamp display
 */
function updateTimestamp(isoTimestamp) {
    const date = new Date(isoTimestamp);
    const secondsAgo = Math.floor((Date.now() - date.getTime()) / 1000);
    const timeString = date.toLocaleTimeString();
    
    document.getElementById('last-updated').textContent = 
        `Updated ${secondsAgo}s ago (${timeString})`;
}

/**
 * Display error message to user
 */
function showError(message) {
    // Could add a toast notification here
    console.error(message);
}
