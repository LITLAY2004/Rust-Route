class RustRouteAuthClient {
    constructor() {
        this.tokenKey = 'rustroute_token';
    }

    getToken() {
        return localStorage.getItem(this.tokenKey);
    }

    setToken(token) {
        if (token) {
            localStorage.setItem(this.tokenKey, token);
        }
    }

    clearToken() {
        localStorage.removeItem(this.tokenKey);
    }
}

class RustRouteAuthUI {
    constructor() {
        this.modal = document.getElementById('login-modal');
        this.form = document.getElementById('login-form');
        this.error = document.getElementById('login-error');
        this.logoutButton = document.getElementById('logout-button');
        this.dashboard = null;
        this.authRequired = false;
    }

    init() {
        if (this.form) {
            this.form.addEventListener('submit', (event) => this.handleSubmit(event));
        }
        if (this.logoutButton) {
            this.logoutButton.addEventListener('click', () => this.logout());
        }
        this.updateUI();
    }

    attachDashboard(dashboard) {
        this.dashboard = dashboard;
        this.updateUI();
    }

    setAuthRequired(required) {
        const normalized = Boolean(required);
        if (this.authRequired === normalized) {
            this.updateUI();
            return;
        }
        this.authRequired = normalized;

        if (!this.authRequired) {
            this.closeModal();
        }

        this.updateUI();
    }

    async handleSubmit(event) {
        event.preventDefault();
        if (!this.form) return;

        this.clearError();
        const formData = new FormData(this.form);
        const payload = {
            username: formData.get('username'),
            password: formData.get('password'),
        };

        try {
            const response = await fetch('/api/auth/login', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    Accept: 'application/json',
                },
                body: JSON.stringify(payload),
            });

            const body = await response.json();
            if (!body.success || !body.data || !body.data.token) {
                throw new Error(body.message || '登录失败');
            }

            window.authClient.setToken(body.data.token);
            this.closeModal();
            this.form.reset();

            if (this.dashboard) {
                this.dashboard.refreshEventStream();
                this.dashboard.loadSystemStatus();
            }
        } catch (error) {
            this.showError(error.message || '登录失败');
        }
    }

    async logout() {
        const token = window.authClient.getToken();
        if (token) {
            try {
                await fetch('/api/auth/logout', {
                    method: 'POST',
                    headers: {
                        Authorization: `Bearer ${token}`,
                        Accept: 'application/json',
                    },
                });
            } catch (error) {
                console.warn('Logout request failed:', error);
            }
        }

        window.authClient.clearToken();
        if (this.dashboard) {
            this.dashboard.refreshEventStream();
        }
        if (this.authRequired) {
            this.openModal();
        } else {
            this.updateUI();
        }
    }

    promptLogin(message) {
        if (!this.authRequired) {
            return;
        }

        if (message) {
            this.showError(message);
        }
        this.openModal();
    }

    showError(message) {
        if (this.error) {
            this.error.textContent = message;
            this.error.classList.remove('hidden');
        }
    }

    clearError() {
        if (this.error) {
            this.error.textContent = '';
            this.error.classList.add('hidden');
        }
    }

    openModal() {
        if (!this.authRequired) {
            return;
        }

        if (this.modal) {
            this.modal.classList.remove('hidden');
        }
        if (this.logoutButton) {
            this.logoutButton.classList.add('hidden');
        }
    }

    closeModal() {
        if (this.modal) {
            this.modal.classList.add('hidden');
        }
        this.clearError();
        this.updateUI();
    }

    updateUI() {
        const token = window.authClient.getToken();

        if (this.logoutButton) {
            this.logoutButton.classList.toggle('hidden', !this.authRequired || !token);
        }

        if (!this.authRequired) {
            if (this.modal) {
                this.modal.classList.add('hidden');
            }
            return;
        }

        if (!token) {
            if (this.modal) {
                this.modal.classList.remove('hidden');
            }
        } else if (this.modal) {
            this.modal.classList.add('hidden');
        }
    }
}

window.authClient = new RustRouteAuthClient();
window.authUI = new RustRouteAuthUI();

class RustRouteAuthError extends Error {
    constructor(message, status) {
        super(message);
        this.name = 'RustRouteAuthError';
        this.status = status;
    }
}

window.RustRouteAuthError = RustRouteAuthError;

window.fetchWithAuth = async function fetchWithAuth(url, options = {}, config = {}) {
    const { silent = false, skipPrompt = false } = config;
    const headers = new Headers(options.headers || {});

    if (!headers.has('Accept')) {
        headers.set('Accept', 'application/json');
    }

    const token = window.authClient?.getToken?.();
    if (token) {
        headers.set('Authorization', `Bearer ${token}`);
    }

    const response = await fetch(url, { ...options, headers });

    if (response.status === 401) {
        window.authClient?.clearToken?.();
        window.authUI?.setAuthRequired?.(true);
        if (!skipPrompt && !silent) {
            window.authUI?.promptLogin('请登录以继续');
        }
        throw new RustRouteAuthError('Unauthorized', 401);
    }

    if (response.status === 403) {
        if (!silent) {
            window.authUI?.showError?.('权限不足');
        }
        throw new RustRouteAuthError('Forbidden', 403);
    }

    return response;
};

document.addEventListener('DOMContentLoaded', () => {
    window.authUI.init();
});
