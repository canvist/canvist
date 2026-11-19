/**
 * The admin dashboard at /admin: sign-in by email, then analytics,
 * coupons and projects in tabs.
 *
 * A sign-in link lands here as `/admin#token=…`. The fragment never reaches
 * a server log; it is traded for a session cookie and wiped from the
 * address bar at once.
 */

import type { Project } from "../../lib/admin.ts";
import { renderReport } from "./analytics.ts";
import * as api from "./api.ts";
import { CouponsTab } from "./coupons.ts";
import { announce, need } from "./dom.ts";
import { renderProjects } from "./projects.ts";

const root = need<HTMLElement>(document, "[data-admin]");
const status = need<HTMLElement>(root, "[data-admin-status]");
const signInView = need<HTMLElement>(root, '[data-view="sign-in"]');
const dashboardView = need<HTMLElement>(root, '[data-view="dashboard"]');
const signedIn = need<HTMLElement>(root, "[data-signed-in]");
const couponsTab = new CouponsTab(need(root, "#panel-coupons"));

let days = 30;
let projects: Project[] = [];

function messageOf(error: unknown): string {
	return error instanceof Error ? error.message : "Something went wrong.";
}

function showSignIn(message?: string): void {
	status.hidden = true;
	dashboardView.hidden = true;
	signedIn.hidden = true;
	signInView.hidden = false;
	if (message) {
		announce(need(signInView, "[data-sign-in-result]"), message, "error");
	}
}

async function loadReport(): Promise<void> {
	const panel = need<HTMLElement>(root, "#panel-analytics");
	panel.setAttribute("aria-busy", "true");
	try {
		renderReport(panel, await api.report(days), days);
	} catch (error) {
		announce(need(panel, "[data-report]"), messageOf(error), "error");
	} finally {
		panel.removeAttribute("aria-busy");
	}
}

async function loadProjects(): Promise<void> {
	const target = need<HTMLElement>(root, "[data-projects]");
	try {
		projects = await api.projects();
		couponsTab.setProjects(projects);
		renderProjects(target, projects, (project) => {
			selectTab("coupons");
			couponsTab.giveTo(project);
		});
	} catch (error) {
		announce(target, messageOf(error), "error");
	}
}

function showDashboard(admin: string): void {
	status.hidden = true;
	signInView.hidden = true;
	dashboardView.hidden = false;
	signedIn.hidden = false;
	need(root, "[data-admin-who]").textContent = admin;
	void loadReport();
	void loadProjects().then(() => couponsTab.load());
}

/** Tabs follow the ARIA pattern: arrow keys move, the selected one is in the tab order. */
function selectTab(name: string, focus = false): void {
	for (const tab of root.querySelectorAll<HTMLButtonElement>('[role="tab"]')) {
		const selected = tab.dataset["tab"] === name;
		tab.setAttribute("aria-selected", String(selected));
		tab.tabIndex = selected ? 0 : -1;
		need<HTMLElement>(root, `#${tab.getAttribute("aria-controls")}`).hidden =
			!selected;
		if (selected && focus) tab.focus();
	}
}

function wireTabs(): void {
	const tabs = [...root.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
	for (const [index, tab] of tabs.entries()) {
		tab.addEventListener("click", () => selectTab(tab.dataset["tab"] ?? ""));
		tab.addEventListener("keydown", (event) => {
			const step = { ArrowRight: 1, ArrowLeft: -1 }[event.key];
			if (step === undefined) return;
			event.preventDefault();
			const next = tabs[(index + step + tabs.length) % tabs.length];
			selectTab(next?.dataset["tab"] ?? "", true);
		});
	}
}

function wireRange(): void {
	for (
		const button of root.querySelectorAll<HTMLButtonElement>("[data-days]")
	) {
		button.addEventListener("click", () => {
			days = Number(button.dataset["days"]);
			for (
				const other of root.querySelectorAll<HTMLButtonElement>("[data-days]")
			) {
				other.setAttribute("aria-pressed", String(other === button));
			}
			void loadReport();
		});
	}
}

function wireSignIn(): void {
	const form = need<HTMLFormElement>(signInView, "[data-sign-in]");
	const result = need<HTMLElement>(signInView, "[data-sign-in-result]");
	form.addEventListener("submit", async (event) => {
		event.preventDefault();
		const email = need<HTMLInputElement>(form, "input[type=email]").value;
		try {
			await api.requestLink(email);
			announce(
				result,
				"If that address can sign in, a link is on its way. It works for 15 minutes.",
			);
		} catch (error) {
			announce(result, messageOf(error), "error");
		}
	});
	need(root, "[data-sign-out]").addEventListener("click", async () => {
		await api.signOut();
		showSignIn();
	});
}

/** canvist Cloud answers 404 on every admin route until admins are configured. */
const NOT_SET_UP = 404;

/** Arrived from a sign-in link: trade its token, then wipe it from the address bar. */
async function signInWithLink(token: string): Promise<void> {
	history.replaceState(null, "", location.pathname);

	try {
		showDashboard(await api.startSession(token));
	} catch {
		// The token is spent either way: an expired or forged link only means asking again.
		showSignIn("That link has expired. Ask for a new one.");
	}
}

/** Arrived directly: resume the session cookie's session, if any. */
async function resumeSession(): Promise<void> {
	try {
		showDashboard(await api.currentAdmin());
	} catch (error) {
		if (error instanceof api.ApiError && error.status === NOT_SET_UP) {
			announce(
				status,
				"Admin is not set up on this server: set CANVIST_ADMIN_EMAILS and CANVIST_ADMIN_URL.",
				"error",
			);
			return;
		}

		showSignIn();
	}
}

wireTabs();
wireRange();
wireSignIn();

function linkToken(): string | null {
	return new URLSearchParams(location.hash.slice(1)).get("token");
}

// A link opened in a tab already on /admin changes only the fragment.
window.addEventListener("hashchange", () => {
	const token = linkToken();
	if (token) void signInWithLink(token);
});

const token = linkToken();
void (token ? signInWithLink(token) : resumeSession());
