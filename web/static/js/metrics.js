class MetricsPage {
    constructor() {
        this.eventSource = null;
        this.eventReconnectTimer = null;
        this.snapshot = null;
    }

    async init() {
        await this.syncAuthRequirement();
        await this.loadMetrics();
        this.setupEventListeners();
        this.initializeEventStream();
        window.authUI?.attachDashboard?.(this);
    }

    async syncAuthRequirement() {
        try {
            const response = await window.fetchWithAuth('/api/status', {}, { silent: true });
            const body = await response.json();
            if (body.success) {
                window.authUI?.setAuthRequired(Boolean(body.data?.auth_required));
            }
        } catch (error) {
            if (!(error instanceof window.RustRouteAuthError)) {
                console.warn('Failed to sync auth status:', error);
            }
        }
    }

    async loadMetrics() {
        const target = document.getElementById('metrics-json');
        try {
            const response = await window.fetchWithAuth('/api/metrics');
            const body = await response.json();
            if (!body.success) {
                throw new Error(body.message || 'Unable to load metrics');
            }

            this.snapshot = body.data || {};
            this.updateCards();
            target.textContent = JSON.stringify(body.data, null, 2);
        } catch (error) {
            if (error instanceof window.RustRouteAuthError) {
                target.textContent = 'Login required to view metrics.';
                return;
            }
            target.textContent = `Error loading metrics: ${error.message}`;
        }
    }

    setupEventListeners() {
        document.getElementById('reload-metrics')?.addEventListener('click', () => {
            this.loadMetrics();
        });
    }

    initializeEventStream() {
        this.closeEventStream();
        const token = window.authClient?.getToken?.();
        const url = token
            ? `/api/events?token=${encodeURIComponent(token)}`
            : '/api/events';

        try {
            this.eventSource = new EventSource(url);
        } catch (error) {
            console.warn('Failed to initialize metrics event stream:', error);
            this.scheduleReconnect();
            return;
        }

        this.eventSource.onmessage = (event) => {
            try {
                const payload = JSON.parse(event.data);
                if (payload.type === 'Metrics') {
                    this.snapshot = payload.data?.snapshot || payload.data;
                    this.updateCards();
                    const target = document.getElementById('metrics-json');
                    if (target && this.snapshot) {
                        target.textContent = JSON.stringify(this.snapshot, null, 2);
                    }
                }
            } catch (err) {
                console.error('Failed to parse metrics event payload:', err);
            }
        };

        this.eventSource.onerror = (error) => {
            console.warn('Metrics event stream error:', error);
            this.scheduleReconnect();
        };
    }

    closeEventStream() {
        if (this.eventSource) {
            this.eventSource.close();
            this.eventSource = null;
        }
        if (this.eventReconnectTimer) {
            clearTimeout(this.eventReconnectTimer);
            this.eventReconnectTimer = null;
        }
    }

    scheduleReconnect() {
        this.closeEventStream();
        this.eventReconnectTimer = setTimeout(() => {
            this.initializeEventStream();
        }, 5000);
    }

    updateCards() {
        const snapshot = this.snapshot || {};
        const setText = (id, value) => {
            const element = document.getElementById(id);
            if (element) {
                element.textContent = value;
            }
        };

        setText('metric-packets-sent', snapshot.packets_sent ?? '—');
        setText('metric-packets-received', snapshot.packets_received ?? '—');
        setText('metric-packets-dropped', snapshot.packets_dropped ?? '—');
        setText('metric-route-count', snapshot.route_count ?? '—');
        setText('metric-routing-updates', snapshot.routing_updates_received ?? '—');
        setText('metric-neighbors', snapshot.neighbor_count ?? '—');
        setText('metric-config-version', snapshot.config_version ?? '—');
        setText('metric-uptime', this.formatDuration(snapshot.uptime_seconds ?? 0));

        const convergence = snapshot.convergence_time_seconds;
        setText('metric-convergence', convergence != null ? `${convergence}s` : '—');
    }

    formatDuration(seconds) {
        const total = Number(seconds) || 0;
        const hours = Math.floor(total / 3600);
        const minutes = Math.floor((total % 3600) / 60);
        const secs = total % 60;
        return `${hours}h ${minutes}m ${secs}s`;
    }

    refreshEventStream() {
        this.initializeEventStream();
    }

    loadSystemStatus() {
        this.loadMetrics();
    }
}

const metricsPage = new MetricsPage();

document.addEventListener('DOMContentLoaded', () => {
    metricsPage.init();
});
