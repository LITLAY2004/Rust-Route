class RoutesPage {
    constructor() {
        this.routes = [];
        this.filteredRoutes = [];
        this.refreshInterval = null;
        this.eventSource = null;
        this.eventReconnectTimer = null;
    }

    async init() {
        await this.syncAuthRequirement();
        await this.loadRoutes();
        this.setupEventListeners();
        this.setupRealTimeUpdates();
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
                console.warn('Failed to synchronize auth requirement:', error);
            }
        }
    }

    async loadRoutes() {
        try {
            const response = await window.fetchWithAuth('/api/routes');
            const body = await response.json();
            if (!body.success) {
                throw new Error(body.message || 'Failed to load routes');
            }

            this.routes = Array.isArray(body.data) ? body.data : [];
            this.filteredRoutes = [...this.routes];
            this.updateRoutesTable();
            this.updateStatistics();
            this.updateInterfaceFilter();
        } catch (error) {
            if (error instanceof window.RustRouteAuthError) {
                this.showEmptyState('Login required to view routes');
                return;
            }
            console.error('Failed to load routes:', error);
            this.showEmptyState('Unable to load routes');
        }
    }

    setupEventListeners() {
        const searchInput = document.getElementById('search-input');
        const metricFilter = document.getElementById('metric-filter');
        const interfaceFilter = document.getElementById('interface-filter');
        const routesTableBody = document.getElementById('routes-tbody');
        const refreshButton = document.getElementById('refresh-button');
        const createRouteForm = document.getElementById('create-route-form');

        searchInput?.addEventListener('input', () => this.filterRoutes());
        metricFilter?.addEventListener('change', () => this.filterRoutes());
        interfaceFilter?.addEventListener('change', () => this.filterRoutes());

        routesTableBody?.addEventListener('click', (event) => {
            const button = event.target.closest('.js-delete-route');
            if (!button) {
                return;
            }
            const destination = button.dataset.destination;
            const mask = button.dataset.mask;
            this.handleDeleteRoute(destination, mask);
        });

        refreshButton?.addEventListener('click', () => {
            this.loadRoutes();
        });

        createRouteForm?.addEventListener('submit', (event) => {
            event.preventDefault();
            this.handleCreateRoute(new FormData(createRouteForm));
        });
    }

    setupRealTimeUpdates() {
        if (this.refreshInterval) {
            clearInterval(this.refreshInterval);
        }
        this.refreshInterval = setInterval(() => {
            this.loadRoutes();
        }, 15000);
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
            console.warn('Failed to initialize event stream:', error);
            this.scheduleEventReconnect();
            return;
        }

        this.eventSource.onmessage = (event) => {
            try {
                const payload = JSON.parse(event.data);
                this.handleEvent(payload);
            } catch (err) {
                console.error('Failed to parse route event payload:', err);
            }
        };

        this.eventSource.onerror = (error) => {
            console.warn('Event stream error:', error);
            this.scheduleEventReconnect();
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

    scheduleEventReconnect() {
        this.closeEventStream();
        this.eventReconnectTimer = setTimeout(() => {
            this.initializeEventStream();
        }, 5000);
    }

    handleEvent(event) {
        switch (event.type) {
            case 'Route':
                // Refresh the route list when we receive an update
                this.loadRoutes();
                break;
            case 'Activity':
                if (event.data?.message?.includes('Configuration reloaded')) {
                    this.loadRoutes();
                }
                break;
            default:
                break;
        }
    }

    filterRoutes() {
        const searchTerm = document.getElementById('search-input')?.value?.toLowerCase() || '';
        const metricFilter = document.getElementById('metric-filter')?.value || '';
        const interfaceFilter = document.getElementById('interface-filter')?.value || '';

        this.filteredRoutes = this.routes.filter((route) => {
            const destination = route.destination?.toLowerCase?.() || '';
            const nextHop = route.next_hop?.toLowerCase?.() || '';
            const iface = route.interface?.toLowerCase?.() || '';

            const matchesSearch = !searchTerm
                || destination.includes(searchTerm)
                || nextHop.includes(searchTerm)
                || iface.includes(searchTerm);

            const metric = Number(route.metric) || 0;
            const matchesMetric = !metricFilter
                || (metricFilter === 'low' && metric <= 5)
                || (metricFilter === 'medium' && metric >= 6 && metric <= 10)
                || (metricFilter === 'high' && metric >= 11);

            const matchesInterface = !interfaceFilter || route.interface === interfaceFilter;

            return matchesSearch && matchesMetric && matchesInterface;
        });

        this.updateRoutesTable();
    }

    updateRoutesTable() {
        const tbody = document.getElementById('routes-tbody');
        if (!tbody) {
            return;
        }

        if (this.filteredRoutes.length === 0) {
            this.showEmptyState('No routes found');
            return;
        }

        const rows = this.filteredRoutes.map((route) => {
            const destination = escapeHtml(route.destination);
            const nextHop = escapeHtml(route.next_hop);
            const metric = Number(route.metric) || 0;
            const iface = escapeHtml(route.interface);
            const age = this.formatAge(route.age_seconds ?? route.age ?? 0);
            const learnedFrom = escapeHtml(route.learned_from) || '—';
            const encodedDestination = encodeURIComponent(route.destination ?? '');
            const encodedMask = encodeURIComponent(route.subnet_mask ?? '');

            return `
                <tr>
                    <td><code>${destination}</code></td>
                    <td><code>${nextHop}</code></td>
                    <td>
                        <span class="metric-badge ${this.getMetricClass(metric)}">${metric}</span>
                    </td>
                    <td><code>${iface}</code></td>
                    <td>${age}</td>
                    <td><code>${learnedFrom}</code></td>
                    <td>
                        <div class="route-actions">
                            <button
                                class="btn btn-danger js-delete-route"
                                data-destination="${encodedDestination}"
                                data-mask="${encodedMask}"
                                title="Delete Route"
                            >
                                <i class="fas fa-trash"></i>
                            </button>
                        </div>
                    </td>
                </tr>
            `;
        });

        tbody.innerHTML = rows.join('');
    }

    updateStatistics() {
        const totalRoutes = this.routes.length;
        const activeRoutes = this.routes.filter((route) => (Number(route.metric) || 0) < 16).length;
        const highMetricRoutes = this.routes.filter((route) => (Number(route.metric) || 0) >= 11).length;
        const avgAgeSeconds = totalRoutes > 0
            ? Math.round(
                  this.routes.reduce((sum, route) => sum + (route.age_seconds ?? route.age ?? 0), 0) /
                      totalRoutes,
              )
            : 0;

        const totalElement = document.getElementById('total-routes');
        const activeElement = document.getElementById('active-routes');
        const highMetricElement = document.getElementById('high-metric-routes');
        const avgAgeElement = document.getElementById('avg-age');

        if (totalElement) totalElement.textContent = totalRoutes;
        if (activeElement) activeElement.textContent = activeRoutes;
        if (highMetricElement) highMetricElement.textContent = highMetricRoutes;
        if (avgAgeElement) avgAgeElement.textContent = this.formatAge(avgAgeSeconds);
    }

    updateInterfaceFilter() {
        const interfaceFilter = document.getElementById('interface-filter');
        if (!interfaceFilter) {
            return;
        }

        const interfaces = [...new Set(this.routes.map((route) => route.interface).filter(Boolean))];
        const currentValue = interfaceFilter.value;
        const options = ['<option value="">All Interfaces</option>'].concat(
            interfaces.map((iface) => `<option value="${iface}">${iface}</option>`),
        );

        interfaceFilter.innerHTML = options.join('');
        interfaceFilter.value = currentValue;
    }

    async handleCreateRoute(formData) {
        const destination = formData.get('destination')?.toString()?.trim();
        const mask = formData.get('mask')?.toString()?.trim();
        const nextHop = formData.get('next_hop')?.toString()?.trim();
        const metricValue = formData.get('metric')?.toString()?.trim();
        const iface = formData.get('interface')?.toString()?.trim();

        if (!destination || !mask || !nextHop || !iface) {
            alert('Destination, subnet mask, next hop and interface are required');
            return;
        }

        const payload = {
            destination,
            mask,
            next_hop: nextHop,
            metric: metricValue ? Number(metricValue) : undefined,
            interface: iface,
        };

        try {
            const response = await window.fetchWithAuth('/api/routes', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(payload),
            });
            const body = await response.json();
            if (!body.success) {
                throw new Error(body.message || 'Failed to create route');
            }

            document.getElementById('create-route-form')?.reset();
            await this.loadRoutes();
            alert('Route created successfully');
        } catch (error) {
            if (error instanceof window.RustRouteAuthError) {
                return;
            }
            console.error('Failed to create route:', error);
            alert(error.message || 'Failed to create route');
        }
    }

    async handleDeleteRoute(destination, mask) {
        if (!destination || !mask) {
            return;
        }

        const decodedDestination = decodeURIComponent(destination || '');
        const decodedMask = decodeURIComponent(mask || '');

        const confirmed = confirm(`Are you sure you want to delete the route to ${decodedDestination}/${decodedMask}?`);
        if (!confirmed) {
            return;
        }

        try {
            const response = await window.fetchWithAuth(
                `/api/routes/${encodeURIComponent(decodedDestination)}/${encodeURIComponent(decodedMask)}`,
                { method: 'DELETE' },
            );
            const body = await response.json();
            if (!body.success) {
                alert(body.message || 'Failed to delete route');
                return;
            }

            await this.loadRoutes();
            alert('Route deleted successfully');
        } catch (error) {
            if (error instanceof window.RustRouteAuthError) {
                return;
            }
            console.error('Failed to delete route:', error);
            alert('Failed to delete route');
        }
    }

    getMetricClass(metric) {
        if (metric <= 5) return 'metric-low';
        if (metric <= 10) return 'metric-medium';
        return 'metric-high';
    }

    formatAge(seconds) {
        const totalSeconds = Number(seconds) || 0;
        if (totalSeconds < 60) return `${totalSeconds}s`;
        if (totalSeconds < 3600) return `${Math.floor(totalSeconds / 60)}m`;
        if (totalSeconds < 86400) return `${Math.floor(totalSeconds / 3600)}h`;
        return `${Math.floor(totalSeconds / 86400)}d`;
    }

    showEmptyState(message) {
        const tbody = document.getElementById('routes-tbody');
        if (!tbody) {
            return;
        }
        tbody.innerHTML = `
            <tr>
                <td colspan="7" style="text-align: center; padding: 2rem;">${escapeHtml(message)}</td>
            </tr>
        `;
    }

    refreshEventStream() {
        this.initializeEventStream();
    }

    loadSystemStatus() {
        this.loadRoutes();
    }
}

function escapeHtml(value) {
    if (value === null || value === undefined) {
        return '';
    }
    return String(value)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

const routesPage = new RoutesPage();

document.addEventListener('DOMContentLoaded', () => {
    routesPage.init();
});

window.refreshRoutes = function refreshRoutes() {
    return routesPage.loadRoutes();
};
