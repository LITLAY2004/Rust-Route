class ConfigPage {
    constructor() {
        this.history = [];
        this.selectedVersion = null;
    }

    async init() {
        await this.syncAuthRequirement();
        await Promise.all([this.loadConfig(), this.loadHistory()]);
        this.setupEventListeners();
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

    async loadConfig() {
        const target = document.getElementById('config-json');
        try {
            const response = await window.fetchWithAuth('/api/config');
            const body = await response.json();
            if (body.success) {
                target.textContent = JSON.stringify(body.data, null, 2);
            } else {
                target.textContent = body.message || 'Unable to load configuration.';
            }
        } catch (error) {
            if (error instanceof window.RustRouteAuthError) {
                target.textContent = 'Login required to view configuration.';
                return;
            }
            target.textContent = `Error loading configuration: ${error.message}`;
        }
    }

    async loadHistory() {
        const historyContainer = document.getElementById('config-history');
        historyContainer.innerHTML = '<li>Loading history...</li>';
        const rollbackButton = document.getElementById('rollback-button');
        if (rollbackButton) {
            rollbackButton.disabled = true;
            rollbackButton.dataset.version = '';
        }

        try {
            const response = await window.fetchWithAuth('/api/config/history');
            const body = await response.json();
            if (!body.success) {
                throw new Error(body.message || 'Failed to load history');
            }

            this.history = Array.isArray(body.data) ? body.data : [];
            if (this.history.length === 0) {
                historyContainer.innerHTML = '<li>No configuration history found.</li>';
                return;
            }

            historyContainer.innerHTML = this.history
                .map((entry) => {
                    const iso = entry.timestamp ? new Date(entry.timestamp).toLocaleString() : 'Unknown time';
                    return `<li><button class="link-button" data-version="${entry.version}">Version ${entry.version} · ${iso}</button></li>`;
                })
                .join('');
        } catch (error) {
            if (error instanceof window.RustRouteAuthError) {
                historyContainer.innerHTML = '<li>Login required to view configuration history.</li>';
                return;
            }
            historyContainer.innerHTML = `<li>Error loading history: ${error.message}</li>`;
        }
    }

    async loadDiff(version) {
        const previousTarget = document.getElementById('config-diff-previous');
        const currentTarget = document.getElementById('config-diff-current');
        previousTarget.textContent = 'Loading diff...';
        currentTarget.textContent = 'Loading diff...';

        try {
            const response = await window.fetchWithAuth(`/api/config/history/${version}/diff`);
            const body = await response.json();
            if (!body.success) {
                throw new Error(body.message || 'Failed to fetch diff');
            }

            previousTarget.textContent = body.data?.previous || 'No previous configuration available.';
            currentTarget.textContent = body.data?.current || 'No current configuration available.';

            this.selectedVersion = version;
            const rollbackButton = document.getElementById('rollback-button');
            rollbackButton.disabled = false;
            rollbackButton.dataset.version = version;
        } catch (error) {
            if (error instanceof window.RustRouteAuthError) {
                previousTarget.textContent = 'Login required to view diff.';
                currentTarget.textContent = '';
                return;
            }
            previousTarget.textContent = `Error loading diff: ${error.message}`;
            currentTarget.textContent = '';
        }
    }

    async rollback(version) {
        if (!version) {
            return;
        }
        const confirmed = confirm(`Rollback configuration to version ${version}?`);
        if (!confirmed) {
            return;
        }

        try {
            const response = await window.fetchWithAuth(
                `/api/config/history/${version}/rollback`,
                {
                    method: 'POST',
                },
            );
            const body = await response.json();
            if (!body.success) {
                throw new Error(body.message || 'Failed to rollback configuration');
            }

            this.showMessage(`Configuration rolled back to version ${version}.`, 'info');
            await Promise.all([this.loadConfig(), this.loadHistory()]);
        } catch (error) {
            if (error instanceof window.RustRouteAuthError) {
                this.showMessage('Login required to rollback configuration.', 'error');
                return;
            }
            this.showMessage(error.message || 'Failed to rollback configuration.', 'error');
        }
    }

    setupEventListeners() {
        document.getElementById('reload-config')?.addEventListener('click', () => {
            this.loadConfig();
        });

        document.getElementById('config-history')?.addEventListener('click', (event) => {
            const button = event.target.closest('button[data-version]');
            if (!button) {
                return;
            }
            const version = Number(button.dataset.version);
            this.loadDiff(version);
        });

        document.getElementById('rollback-button')?.addEventListener('click', (event) => {
            const version = Number(event.currentTarget.dataset.version);
            if (Number.isFinite(version)) {
                this.rollback(version);
            }
        });
    }

    showMessage(message, type = 'info') {
        const container = document.getElementById('config-message');
        if (!container) {
            return;
        }
        container.textContent = message;
        container.dataset.type = type;
        container.classList.remove('hidden');
        setTimeout(() => container.classList.add('hidden'), 5000);
    }

    refreshEventStream() {
        // No event stream on config page, but ensure auth refresh reloads data
        this.loadConfig();
        this.loadHistory();
    }

    loadSystemStatus() {
        this.loadConfig();
    }
}

const configPage = new ConfigPage();

document.addEventListener('DOMContentLoaded', () => {
    configPage.init();
});
