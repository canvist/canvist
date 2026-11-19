/**
 * The admin dashboard's calls to canvist Cloud's `/v1/admin` routes, on
 * this origin (Caddy proxies them). The session is an HttpOnly cookie the
 * browser sends by itself.
 *
 * Responses come from canvist's own server, so they are trusted to match
 * `src/lib/admin.ts`; each is still checked at its top level, so a
 * mismatched server fails loudly instead of drawing nonsense.
 */

import type { Coupon, NewCoupon, Project, Report } from "../../lib/admin.ts";

/** A request canvist Cloud refused. */
export class ApiError extends Error {
	constructor(
		readonly status: number,
		readonly code: string,
		message: string,
	) {
		super(message);
		this.name = "ApiError";
	}
}

type Json = Record<string, unknown>;

function isRecord(value: unknown): value is Json {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

async function call(
	method: string,
	path: string,
	body?: unknown,
): Promise<Json | null> {
	const init: RequestInit = { method, credentials: "same-origin" };
	if (body !== undefined) {
		init.body = JSON.stringify(body);
		init.headers = { "Content-Type": "application/json" };
	}
	const response = await fetch(path, init);
	const text = await response.text();
	const parsed: unknown = text ? JSON.parse(text) : null;

	if (!response.ok) {
		const error = isRecord(parsed) && isRecord(parsed["error"])
			? parsed["error"]
			: {};
		const code = typeof error["code"] === "string" ? error["code"] : "unknown";
		const message = typeof error["message"] === "string"
			? error["message"]
			: `canvist Cloud answered ${response.status}`;
		throw new ApiError(response.status, code, message);
	}

	return isRecord(parsed) ? parsed : null;
}

function field<T>(
	body: Json | null,
	key: string,
	check: (value: unknown) => boolean,
): T {
	const value = body?.[key];
	if (!check(value)) {
		throw new ApiError(502, "malformed", `canvist Cloud sent no ${key}`);
	}
	return value as T;
}

const isString = (value: unknown) => typeof value === "string";

/** Who is signed in, or an `ApiError` (401 signed out, 404 not set up). */
export async function currentAdmin(): Promise<string> {
	return field<string>(
		await call("GET", "/v1/admin/session"),
		"admin",
		isString,
	);
}

/** Ask for a sign-in link. The answer never says whether one was sent. */
export async function requestLink(email: string): Promise<void> {
	await call("POST", "/v1/admin/sign-in", { email });
}

/** Trade a link's token for a session; returns who signed in. */
export async function startSession(token: string): Promise<string> {
	return field<string>(
		await call("POST", "/v1/admin/session", { token }),
		"admin",
		isString,
	);
}

export async function signOut(): Promise<void> {
	await call("DELETE", "/v1/admin/session");
}

export async function report(days: number): Promise<Report> {
	const body = await call("GET", `/v1/admin/analytics?days=${days}`);
	field(body, "days", Array.isArray);
	field(body, "summary", isRecord);
	return body as unknown as Report;
}

export async function projects(): Promise<Project[]> {
	return field<Project[]>(
		await call("GET", "/v1/admin/projects"),
		"projects",
		Array.isArray,
	);
}

export async function coupons(): Promise<Coupon[]> {
	return field<Coupon[]>(
		await call("GET", "/v1/admin/coupons"),
		"coupons",
		Array.isArray,
	);
}

export interface Created {
	coupon: Coupon;
	applied: boolean;
}

export async function createCoupon(coupon: NewCoupon): Promise<Created> {
	const body = await call("POST", "/v1/admin/coupons", coupon);
	return {
		coupon: field<Coupon>(body, "coupon", isRecord),
		applied: body?.["applied_to_subscription"] === true,
	};
}

export async function deleteCoupon(id: string): Promise<void> {
	await call("DELETE", `/v1/admin/coupons/${encodeURIComponent(id)}`);
}
