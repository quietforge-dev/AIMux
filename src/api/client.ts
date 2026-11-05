let base = import.meta.env.VITE_API_BASE || window.location.origin;
let authToken: string | undefined;

export const setAuthToken = (value: string) => {
  authToken = value.trim() || undefined;
};

export const clearAuthToken = () => {
  authToken = undefined;
};

export const setApiBase = (value: string) => {
  const normalized = value.trim().replace(/\/+$/, '');
  if (normalized) base = normalized;
};

export async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set('Content-Type', 'application/json');
  if (authToken) headers.set('Authorization', `Bearer ${authToken}`);
  const response = await fetch(`${base}${path}`, {
    headers,
    ...init,
  });
  if (response.status === 401 && path !== '/api/auth/login') {
    clearAuthToken();
    window.dispatchEvent(new CustomEvent('aimux-auth-required'));
  }
  if (!response.ok) {
    const detail = await response.text();
    throw new Error(detail || `${response.status} ${response.statusText}`);
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}
export const get = <T>(path: string) => request<T>(path);
export const post = <T>(path: string, body?: unknown, init: RequestInit = {}) =>
  request<T>(path, {
    ...init,
    method: 'POST',
    body: body === undefined ? undefined : JSON.stringify(body),
  });
export const put = <T>(path: string, body: unknown) =>
  request<T>(path, { method: 'PUT', body: JSON.stringify(body) });
export const del = <T>(path: string) => request<T>(path, { method: 'DELETE' });
