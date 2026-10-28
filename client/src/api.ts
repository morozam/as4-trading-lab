export class ApiError extends Error {
  status: number;
  constructor(message: string, status: number) { super(message); this.status = status; }
}
export async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const response = await fetch(path, {credentials: 'same-origin', ...options});
  if (!response.ok) {
    const payload = await response.json().catch(() => ({}));
    throw new ApiError(payload.error || `Request failed (${response.status})`, response.status);
  }
  return response.status === 204 ? undefined as T : response.json();
}
export function command<T>(csrf: string, path: string, body?: unknown, method = 'POST'): Promise<T> {
  return request<T>(path, {method, headers: {'Content-Type': 'application/json', 'X-CSRF-Token': csrf}, body: JSON.stringify(body ?? {})});
}
export function key(): string {
  if (typeof crypto.randomUUID === 'function') return crypto.randomUUID();
  // getRandomValues is also available on HTTP LAN origins; randomUUID is not.
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, value => value.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0,8)}-${hex.slice(8,12)}-${hex.slice(12,16)}-${hex.slice(16,20)}-${hex.slice(20)}`;
}
export function dateTime(value: string | null | undefined): string { return value ? new Date(value).toLocaleString('en-GB', {timeZone:'Europe/Amsterdam',timeZoneName:'short',year:'numeric',month:'short',day:'2-digit',hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false}) : 'No callback yet'; }
export function number(value: number): string { return value.toLocaleString(); }
