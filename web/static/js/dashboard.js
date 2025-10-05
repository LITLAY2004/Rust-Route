// Dashboard JavaScript
class RustRouteDashboard {
    constructor() {
        this.apiBase = '/api';
        this.updateInterval = 5000; // 5 seconds
        this.charts = {};
        this.maxDataPoints = 20;
        this.trafficHistory = {
            labels: [],
            inbound: [],
            outbound: [],
        };
        this.lastMetricSnapshot = null;
        this.eventSource = null;
    }

    initialize() {
        this.initializeCharts();
        this.refreshEventStream();
        this.startRealTimeUpdates();
        if (window.authUI) {
            window.authUI.attachDashboard(this);
        }
    }

    initializeEventStream() {
        try {
            const token = window.authClient?.getToken?.();
            const url = token
                ? `/api/events?token=${encodeURIComponent(token)}`
                : '/api/events';
            this.eventSource = new EventSource(url);
            this.eventSource.onmessage = (event) => {
                try {
                    const payload = JSON.parse(event.data);
                    this.handleEvent(payload);
                } catch (err) {
                    console.error('Failed to parse event payload', err);
                }
            };
            this.eventSource.onerror = (error) => {
                console.warn('Event stream error:', error);
            };
        } catch (error) {
            console.error('Failed to initialize event stream:', error);
        }
    }

    refreshEventStream() {
        if (this.eventSource) {
            this.eventSource.close();
            this.eventSource = null;
        }
        this.initializeEventStream();
    }

    handleEvent(event) {
        switch (event.type) {
            case 'Metrics':
                if (event.data && event.data.snapshot) {
                    this.handleMetricsEvent(event.data.snapshot);
                }
                break;
            case 'Route':
                this.handleRouteEvent(event.data);
                break;
            case 'Activity':
                this.handleActivityEvent(event.data);
                break;
            default:
                console.debug('Unhandled event type', event);
        }
    }

    async fetchAPI(endpoint, options = {}) {
        const response = await window.fetchWithAuth(`${this.apiBase}${endpoint}`, options);
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        return await response.json();
    }

    async loadSystemStatus() {
        try {
            const response = await this.fetchAPI('/status');
            if (response.success) {
                this.updateSystemStatus(response.data);
            }
        } catch (error) {
            if (error instanceof window.RustRouteAuthError) {
                return;
            }
            console.error('Failed to load system status:', error);
            this.showError('无法加载系统状态');
        }
    }

    updateSystemStatus(status) {
        if (window.authUI?.setAuthRequired) {
            window.authUI.setAuthRequired(Boolean(status.auth_required));
        }

        const uptimeElement = document.getElementById('uptime');
        if (uptimeElement) {
            uptimeElement.textContent = this.formatUptime(status.uptime_seconds ?? 0);
        }

        const routeCountElement = document.getElementById('route-count');
        if (routeCountElement) {
            routeCountElement.textContent = status.route_count ?? 0;
        }

        const interfaceCountElement = document.getElementById('interface-count');
        if (interfaceCountElement) {
            interfaceCountElement.textContent = (status.interfaces || []).length;
        }

        const memoryUsageElement = document.getElementById('memory-usage');
        if (memoryUsageElement && typeof status.memory_usage === 'number') {
            memoryUsageElement.textContent = this.formatBytes(status.memory_usage);
        }

        this.updateInterfaceGrid(status.interfaces || []);

        if (status.metrics) {
            this.handleMetricsEvent(status.metrics);
        }

        if (status.router_stats && status.router_stats.table_breakdown) {
            this.updateRouteChart(status.router_stats.table_breakdown);
        }
    }

    async loadInterfaces() {
        try {
            const response = await this.fetchAPI('/interfaces');
            if (response.success) {
                this.updateInterfaceGrid(response.data);
            }
        } catch (error) {
            if (error instanceof window.RustRouteAuthError) {
                return;
            }
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
                    <div class="interface-status ${(iface.status || 'unknown').toLowerCase()}">${iface.status || 'unknown'}</div>
                </div>
                <div class="interface-details">
                    <div>IP: ${iface.address ?? '-'}</div>
                </div>
                <div class="interface-stats">
                    <div>TX: ${this.formatBytes(iface.bytes_sent ?? 0)}</div>
                    <div>RX: ${this.formatBytes(iface.bytes_received ?? 0)}</div>
                    <div>Sent: ${iface.packets_sent ?? 0}</div>
                    <div>Recv: ${iface.packets_received ?? 0}</div>
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
                labels: [],
                datasets: [{
                    label: 'Packets In',
                    data: [],
                    borderColor: 'rgb(37, 99, 235)',
                    backgroundColor: 'rgba(37, 99, 235, 0.1)',
                    tension: 0.4
                }, {
                    label: 'Packets Out',
                    data: [],
                    borderColor: 'rgb(16, 185, 129)',
                    backgroundColor: 'rgba(16, 185, 129, 0.1)',
                    tension: 0.4
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    x: {
                        ticks: {
                            font: { size: 12 },
                            maxTicksLimit: 10
                        },
                        grid: {
                            display: true,
                            color: 'rgba(0, 0, 0, 0.05)'
                        }
                    },
                    y: {
                        beginAtZero: true,
                        ticks: {
                            font: { size: 12 },
                            callback: value => `${value} pkts`,
                            maxTicksLimit: 8
                        },
                        grid: {
                            display: true,
                            color: 'rgba(0, 0, 0, 0.05)'
                        }
                    }
                },
                plugins: {
                    legend: {
                        position: 'top',
                        labels: {
                            font: { size: 13 },
                            padding: 20,
                            usePointStyle: true
                        }
                    }
                },
                elements: {
                    point: {
                        radius: 3,
                        hoverRadius: 6
                    },
                    line: {
                        borderWidth: 2
                    }
                },
                animation: { duration: 600 }
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
                    data: [0, 0, 0],
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
                        labels: {
                            font: { size: 12 },
                            padding: 15,
                            usePointStyle: true
                        }
                    }
                },
                cutout: '60%',
                animation: { duration: 600 }
            }
        });
    }

    updateRouteChart(breakdown) {
        if (!this.charts.route) return;
        const dataset = this.charts.route.data.datasets[0];
        dataset.data = [
            breakdown.learned_routes ?? 0,
            breakdown.static_routes ?? 0,
            breakdown.direct_routes ?? 0,
        ];
        this.charts.route.update('none');
    }

    startRealTimeUpdates() {
        this.loadSystemStatus();
        this.loadInterfaces();

        setInterval(() => {
            this.loadSystemStatus();
        }, this.updateInterval);

        setInterval(() => {
            this.loadInterfaces();
        }, this.updateInterval * 2);
    }

    handleMetricsEvent(snapshot) {
        if (!this.charts.traffic) {
            return;
        }

        if (!this.lastMetricSnapshot) {
            this.lastMetricSnapshot = snapshot;
            return;
        }

        const inboundDelta = Math.max(
            0,
            (snapshot.packets_received ?? 0) - (this.lastMetricSnapshot.packets_received ?? 0)
        );
        const outboundDelta = Math.max(
            0,
            (snapshot.packets_sent ?? 0) - (this.lastMetricSnapshot.packets_sent ?? 0)
        );
        this.lastMetricSnapshot = snapshot;

        const label = this.formatTime(new Date());
        this.trafficHistory.labels.push(label);
        this.trafficHistory.inbound.push(inboundDelta);
        this.trafficHistory.outbound.push(outboundDelta);

        if (this.trafficHistory.labels.length > this.maxDataPoints) {
            this.trafficHistory.labels.shift();
            this.trafficHistory.inbound.shift();
            this.trafficHistory.outbound.shift();
        }

        const chart = this.charts.traffic;
        chart.data.labels = [...this.trafficHistory.labels];
        chart.data.datasets[0].data = [...this.trafficHistory.inbound];
        chart.data.datasets[1].data = [...this.trafficHistory.outbound];
        chart.update('none');
    }

    handleRouteEvent(event) {
        if (!event) return;
        const message = `Route ${event.destination}/${event.subnet_mask} metric ${event.metric} via ${event.next_hop}`;
        this.addActivityItem({
            time: this.formatTime(new Date()),
            level: 'info',
            message,
        });
    }

    handleActivityEvent(event) {
        if (!event) return;
        const level = (event.level || 'Info').toString().toLowerCase();
        this.addActivityItem({
            time: this.formatTime(new Date()),
            level,
            message: event.message || '',
        });
    }

    addActivityItem({ time, level, message }) {
        const activityLog = document.getElementById('activity-log');
        if (!activityLog) return;

        const activityItem = document.createElement('div');
        activityItem.className = 'activity-item';
        activityItem.innerHTML = `
            <span class="activity-time">${time}</span>
            <span class="activity-type ${level}">${level.toUpperCase()}</span>
            <span class="activity-message">${message}</span>
        `;

        activityLog.insertBefore(activityItem, activityLog.firstChild);
        while (activityLog.children.length > 10) {
            activityLog.removeChild(activityLog.lastChild);
        }
    }

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
        if (!bytes) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
    }

    formatTime(date) {
        return date.toTimeString().split(' ')[0];
    }

    showError(message) {
        console.error(message);
    }
}

function initializeDashboard() {
    window.dashboard = new RustRouteDashboard();
    window.dashboard.initialize();
}

if (typeof module !== 'undefined' && module.exports) {
    module.exports = RustRouteDashboard;
}
