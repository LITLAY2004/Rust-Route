// Dashboard JavaScript
class RustRouteDashboard {
    constructor() {
        this.apiBase = '/api';
        this.updateInterval = 5000; // 5 seconds
        this.charts = {};
        this.ws = null;
        
        this.initializeWebSocket();
    }

    async initializeWebSocket() {
        try {
            this.ws = new WebSocket(`ws://${window.location.host}/ws`);
            
            this.ws.onopen = () => {
                console.log('WebSocket connected');
            };
            
            this.ws.onmessage = (event) => {
                const data = JSON.parse(event.data);
                this.handleRealtimeUpdate(data);
            };
            
            this.ws.onclose = () => {
                console.log('WebSocket disconnected, attempting to reconnect...');
                setTimeout(() => this.initializeWebSocket(), 5000);
            };
            
            this.ws.onerror = (error) => {
                console.error('WebSocket error:', error);
            };
        } catch (error) {
            console.error('Failed to initialize WebSocket:', error);
        }
    }

    handleRealtimeUpdate(data) {
        switch (data.type) {
            case 'system_status':
                this.updateSystemStatus(data.payload);
                break;
            case 'route_update':
                this.updateRouteInfo(data.payload);
                break;
            case 'activity':
                this.addActivityItem(data.payload);
                break;
        }
    }

    async fetchAPI(endpoint) {
        try {
            const response = await fetch(`${this.apiBase}${endpoint}`);
            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }
            return await response.json();
        } catch (error) {
            console.error(`API fetch error for ${endpoint}:`, error);
            throw error;
        }
    }

    async loadSystemStatus() {
        try {
            const response = await this.fetchAPI('/status');
            if (response.success) {
                this.updateSystemStatus(response.data);
            }
        } catch (error) {
            console.error('Failed to load system status:', error);
            this.showError('Failed to load system status');
        }
    }

    updateSystemStatus(status) {
        // Update uptime
        const uptimeElement = document.getElementById('uptime');
        if (uptimeElement) {
            uptimeElement.textContent = this.formatUptime(status.uptime);
        }

        // Update route count
        const routeCountElement = document.getElementById('route-count');
        if (routeCountElement) {
            routeCountElement.textContent = status.route_count.toString();
        }

        // Update interface count
        const interfaceCountElement = document.getElementById('interface-count');
        if (interfaceCountElement) {
            interfaceCountElement.textContent = status.interfaces.length.toString();
        }

        // Update memory usage
        const memoryUsageElement = document.getElementById('memory-usage');
        if (memoryUsageElement) {
            memoryUsageElement.textContent = this.formatBytes(status.memory_usage);
        }

        // Update interface grid
        this.updateInterfaceGrid(status.interfaces);
    }

    async loadInterfaces() {
        try {
            const response = await this.fetchAPI('/interfaces');
            if (response.success) {
                this.updateInterfaceGrid(response.data);
            }
        } catch (error) {
            console.error('Failed to load interfaces:', error);
        }
    }

    updateInterfaceGrid(interfaces) {
        const grid = document.getElementById('interface-grid');
        if (!grid) return;

        grid.innerHTML = interfaces.map(iface => `
            <div class="interface-card">
                <div class="interface-header">
                    <div class="interface-name">${iface.name}</div>
                    <div class="interface-status ${iface.status.toLowerCase()}">${iface.status}</div>
                </div>
                <div class="interface-details">
                    <div>IP: ${iface.address}</div>
                </div>
                <div class="interface-stats">
                    <div>TX: ${this.formatBytes(iface.bytes_sent)}</div>
                    <div>RX: ${this.formatBytes(iface.bytes_received)}</div>
                    <div>Sent: ${iface.packets_sent}</div>
                    <div>Recv: ${iface.packets_received}</div>
                </div>
            </div>
        `).join('');
    }

    initializeCharts() {
        this.initializeTrafficChart();
        this.initializeRouteChart();
    }

    initializeTrafficChart() {
        const ctx = document.getElementById('trafficChart');
        if (!ctx) return;

        this.charts.traffic = new Chart(ctx, {
            type: 'line',
            data: {
                labels: this.generateTimeLabels(20),
                datasets: [{
                    label: 'Bytes In',
                    data: this.generateRandomData(20, 1000, 5000),
                    borderColor: 'rgb(37, 99, 235)',
                    backgroundColor: 'rgba(37, 99, 235, 0.1)',
                    tension: 0.4
                }, {
                    label: 'Bytes Out',
                    data: this.generateRandomData(20, 800, 4000),
                    borderColor: 'rgb(16, 185, 129)',
                    backgroundColor: 'rgba(16, 185, 129, 0.1)',
                    tension: 0.4
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: {
                        beginAtZero: true,
                        ticks: {
                            callback: (value) => this.formatBytes(value)
                        }
                    }
                },
                plugins: {
                    legend: {
                        position: 'top',
                    }
                }
            }
        });
    }

    initializeRouteChart() {
        const ctx = document.getElementById('routeChart');
        if (!ctx) return;

        this.charts.route = new Chart(ctx, {
            type: 'doughnut',
            data: {
                labels: ['Learned Routes', 'Static Routes', 'Connected Routes'],
                datasets: [{
                    data: [15, 5, 8],
                    backgroundColor: [
                        'rgb(37, 99, 235)',
                        'rgb(16, 185, 129)',
                        'rgb(245, 158, 11)'
                    ],
                    borderWidth: 0
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        position: 'bottom',
                    }
                }
            }
        });
    }

    addActivityItem(activity) {
        const activityLog = document.getElementById('activity-log');
        if (!activityLog) return;

        const activityItem = document.createElement('div');
        activityItem.className = 'activity-item';
        activityItem.innerHTML = `
            <span class="activity-time">${this.formatTime(new Date())}</span>
            <span class="activity-type ${activity.type.toLowerCase()}">${activity.type.toUpperCase()}</span>
            <span class="activity-message">${activity.message}</span>
        `;

        // Insert at the beginning
        activityLog.insertBefore(activityItem, activityLog.firstChild);

        // Keep only the last 10 items
        while (activityLog.children.length > 10) {
            activityLog.removeChild(activityLog.lastChild);
        }
    }

    startRealTimeUpdates() {
        // Initial load
        this.loadSystemStatus();
        this.loadInterfaces();

        // Set up periodic updates
        setInterval(() => {
            this.loadSystemStatus();
            this.updateTrafficChart();
        }, this.updateInterval);

        // Update interfaces less frequently
        setInterval(() => {
            this.loadInterfaces();
        }, this.updateInterval * 2);
    }

    updateTrafficChart() {
        if (!this.charts.traffic) return;

        const chart = this.charts.traffic;
        
        // Remove first data point and add new one
        chart.data.labels.shift();
        chart.data.labels.push(this.formatTime(new Date()));
        
        chart.data.datasets.forEach(dataset => {
            dataset.data.shift();
            dataset.data.push(Math.floor(Math.random() * 5000) + 1000);
        });
        
        chart.update('none');
    }

    // Utility functions
    formatUptime(seconds) {
        const days = Math.floor(seconds / 86400);
        const hours = Math.floor((seconds % 86400) / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);
        
        if (days > 0) {
            return `${days}d ${hours}h ${minutes}m`;
        } else if (hours > 0) {
            return `${hours}h ${minutes}m`;
        } else {
            return `${minutes}m`;
        }
    }

    formatBytes(bytes) {
        if (bytes === 0) return '0 B';
        
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    }

    formatTime(date) {
        return date.toTimeString().split(' ')[0];
    }

    generateTimeLabels(count) {
        const labels = [];
        const now = new Date();
        
        for (let i = count - 1; i >= 0; i--) {
            const time = new Date(now.getTime() - i * 60000); // 1 minute intervals
            labels.push(this.formatTime(time));
        }
        
        return labels;
    }

    generateRandomData(count, min, max) {
        return Array.from({ length: count }, () => 
            Math.floor(Math.random() * (max - min + 1)) + min
        );
    }

    showError(message) {
        console.error(message);
        // Could implement a toast notification system here
    }

    showSuccess(message) {
        console.log(message);
        // Could implement a toast notification system here
    }
}

// Global functions
function initializeDashboard() {
    window.dashboard = new RustRouteDashboard();
    window.dashboard.initializeCharts();
}

function startRealTimeUpdates() {
    window.dashboard.startRealTimeUpdates();
}

// Export for module use
if (typeof module !== 'undefined' && module.exports) {
    module.exports = RustRouteDashboard;
}
