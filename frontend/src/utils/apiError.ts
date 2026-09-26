export async function apiError(response: Response): Promise<Error> {
  const raw: unknown = await response.json().catch(() => null);
  const body = raw !== null && typeof raw === "object" ? raw as { error?: unknown } : {};
  const message = typeof body.error === "string" ? body.error :
    response.status === 429 ? "Scan usage limit reached." :
    response.status === 503 ? "The service is busy or waking up." :
    response.status === 504 ? "The scan timed out. Try fewer wallets." :
    "The service could not complete this request. Please retry later.";
  const retry = Number(response.headers.get("Retry-After"));
  return new Error(message + (Number.isFinite(retry) && retry > 0
    ? ` Retry in about ${Math.ceil(retry / 60)} minute(s).` : ""));
}
