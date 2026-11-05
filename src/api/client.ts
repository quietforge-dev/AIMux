let base = import.meta.env.VITE_API_BASE || window.location.origin;
const AUTH_TOKEN_STORAGE_KEY = 'aimux-session-token';

const readAuthToken = (): string | undefined => {
  try {
    return window.localStorage.getItem(AUTH_TOKEN_STORAGE_KEY)?.trim() || undefined;
  } catch {
    return undefined;
  }
};

let authToken = readAuthToken();
let desktopToken: string | undefined;
let authEpoch = 0;
export const setDesktopToken = (value: string) => {
  desktopToken = value;
  authEpoch++;
};

export const setAuthToken = (value: string) => {
  authEpoch++;
  authToken = value.trim() || undefined;
  try {
    if (authToken) window.localStorage.setItem(AUTH_TOKEN_STORAGE_KEY, authToken);
    else window.localStorage.removeItem(AUTH_TOKEN_STORAGE_KEY);
  } catch {
    // 禁用浏览器存储时，当前页面仍可使用内存中的会话。
  }
};

export const clearAuthToken = () => {
  setAuthToken('');
};

export const setApiBase = (value: string) => {
  const normalized = value.trim().replace(/\/+$/, '');
  if (normalized) base = normalized;
};

export async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set('Content-Type', 'application/json');
  const epoch = authEpoch;
  const token = desktopToken || authToken;
  if (token) headers.set('Authorization', `Bearer ${token}`);
  const response = await fetch(`${base}${path}`, {
    ...init,
    headers,
  });
  if (epoch !== authEpoch) throw new Error('登录用户已变化，请重新操作');
  if (response.status === 401 && path !== '/api/auth/login') {
    clearAuthToken();
    window.dispatchEvent(new CustomEvent('aimux-auth-required'));
  }
  if (!response.ok) {
    const detail = await response.text();
    throw new Error(detail || `${response.status} ${response.statusText}`);
  }
  if (response.status === 204) return undefined as T;
  const value = (await response.json()) as T;
  if (epoch !== authEpoch) throw new Error('登录用户已变化，请重新操作');
  return value;
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
