/**
 * The coupons tab: a form that makes a public or private coupon (with
 * presets for the offers canvist makes most), and the coupons in Stripe.
 */

import {
	type Coupon,
	describeDiscount,
	endOfDay,
	formatCount,
	type NewCoupon,
	PRESETS,
	type Project,
} from "../../lib/admin.ts";
import * as api from "./api.ts";
import { announce, h, need } from "./dom.ts";

/** The form's controls, found once. */
interface Controls {
	form: HTMLFormElement;
	status: HTMLElement;
	list: HTMLElement;
	name: HTMLInputElement;
	amount: HTMLInputElement;
	currency: HTMLInputElement;
	months: HTMLInputElement;
	code: HTMLInputElement;
	project: HTMLSelectElement;
	maxRedemptions: HTMLInputElement;
	expires: HTMLInputElement;
}

function controls(root: HTMLElement): Controls {
	return {
		form: need(root, "[data-coupon-form]"),
		status: need(root, "[data-coupon-status]"),
		list: need(root, "[data-coupon-list]"),
		name: need(root, "#coupon-name"),
		amount: need(root, "#coupon-amount"),
		currency: need(root, "#coupon-currency"),
		months: need(root, "#coupon-months"),
		code: need(root, "#coupon-code"),
		project: need(root, "#coupon-project"),
		maxRedemptions: need(root, "#coupon-max"),
		expires: need(root, "#coupon-expires"),
	};
}

function choice(form: HTMLFormElement, name: string): string {
	const value = new FormData(form).get(name);
	return typeof value === "string" ? value : "";
}

function setChoice(form: HTMLFormElement, name: string, value: string): void {
	for (
		const input of form.querySelectorAll<HTMLInputElement>(
			`input[name="${name}"]`,
		)
	) {
		input.checked = input.value === value;
	}
}

/** Show only the fields the current choices use. */
function sync(ui: Controls): void {
	const kind = choice(ui.form, "kind");
	const duration = choice(ui.form, "duration");
	const visibility = choice(ui.form, "visibility");
	for (const group of ui.form.querySelectorAll<HTMLElement>("[data-when]")) {
		const [name, value] = (group.dataset["when"] ?? "").split("=");
		const current = { kind, duration, visibility }[name ?? ""];
		group.hidden = current !== value;
	}
	ui.currency.required = kind === "amount";
	ui.months.required = duration === "repeating";
	ui.project.required = visibility === "private";
}

/** Cents per unit of an `amount_off` currency. */
const CENTS = 100;

function discountOf(ui: Controls, amount: number) {
	if (choice(ui.form, "kind") !== "amount") return { percent_off: amount };

	return {
		amount_off: Math.round(amount * CENTS),
		currency: ui.currency.value.trim().toLowerCase(),
	};
}

function timingOf(ui: Controls) {
	const duration = choice(ui.form, "duration");
	if (duration === "repeating") {
		return { duration, duration_in_months: Number(ui.months.value) } as const;
	}

	return { duration: duration === "forever" ? "forever" : "once" } as const;
}

function audienceOf(ui: Controls) {
	if (choice(ui.form, "visibility") === "private") {
		return { visibility: "private", project: ui.project.value } as const;
	}

	const code = ui.code.value.trim().toUpperCase();

	return code
		? { visibility: "public", code } as const
		: { visibility: "public" } as const;
}

function limitsOf(
	ui: Controls,
): { max_redemptions?: number; expires_at?: number } {
	const limits: { max_redemptions?: number; expires_at?: number } = {};
	const expires = endOfDay(ui.expires.value);
	if (ui.maxRedemptions.value) {
		limits.max_redemptions = Number(ui.maxRedemptions.value);
	}
	if (expires !== undefined) limits.expires_at = expires;

	return limits;
}

/** The coupon the form describes, or a problem to show. */
function read(ui: Controls): NewCoupon | string {
	const name = ui.name.value.trim();
	const amount = Number(ui.amount.value);
	const audience = audienceOf(ui);
	if (!name) return "Give the coupon a name.";
	if (!(amount > 0)) return "Enter how much it takes off.";
	if (audience.visibility === "private" && !audience.project) {
		return "Pick the project this coupon is for.";
	}

	return {
		name,
		...discountOf(ui, amount),
		...timingOf(ui),
		...audience,
		...limitsOf(ui),
	};
}

function applyPreset(ui: Controls, index: number): void {
	const preset = PRESETS[index];
	if (!preset) return;
	const { coupon } = preset;
	ui.name.value = coupon.name;
	if ("percent_off" in coupon) {
		setChoice(ui.form, "kind", "percent");
		ui.amount.value = String(coupon.percent_off);
	} else {
		setChoice(ui.form, "kind", "amount");
		ui.amount.value = String(coupon.amount_off / CENTS);
		ui.currency.value = coupon.currency;
	}
	setChoice(ui.form, "duration", coupon.duration);
	ui.months.value = String(coupon.duration_in_months ?? "");
	setChoice(ui.form, "visibility", coupon.visibility);
	sync(ui);
	ui.name.focus();
}

function audience(coupon: Coupon, names: Map<string, string>): string {
	const codes = coupon.codes.map((code) => code.code).join(", ");
	if (coupon.visibility === "private") {
		const project = coupon.project
			? names.get(coupon.project) ?? coupon.project
			: "one customer";
		return `Private to ${project}${codes ? ` · ${codes}` : ""}`;
	}
	return codes ? `Public · ${codes}` : "Public";
}

function couponItem(
	coupon: Coupon,
	names: Map<string, string>,
	remove: (coupon: Coupon) => void,
): HTMLElement {
	const confirm = h("span", { class: "coupon__confirm", hidden: true });
	const deleteButton = h(
		"button",
		{ type: "button", class: "link-button" },
		"Delete",
	);
	const yes = h("button", {
		type: "button",
		class: "link-button link-button--danger",
	}, "Delete it");
	const no = h("button", { type: "button", class: "link-button" }, "Keep");
	confirm.append("Delete this coupon? ", yes, " ", no);
	deleteButton.addEventListener("click", () => {
		deleteButton.hidden = true;
		confirm.hidden = false;
		no.focus();
	});
	no.addEventListener("click", () => {
		confirm.hidden = true;
		deleteButton.hidden = false;
		deleteButton.focus();
	});
	yes.addEventListener("click", () => remove(coupon));

	return h(
		"li",
		{ class: "coupon", "data-visibility": coupon.visibility },
		h(
			"div",
			{ class: "coupon__main" },
			h("p", { class: "coupon__name" }, coupon.name ?? coupon.id),
			h("p", { class: "coupon__discount" }, describeDiscount(coupon)),
			h("p", { class: "coupon__audience" }, audience(coupon, names)),
		),
		h(
			"div",
			{ class: "coupon__side" },
			h(
				"p",
				{ class: "coupon__redeemed code-label" },
				`${formatCount(coupon.times_redeemed)} redeemed`,
			),
			coupon.valid
				? null
				: h("p", { class: "coupon__expired code-label" }, "No longer valid"),
			deleteButton,
			confirm,
		),
	);
}

/** The coupons tab, wired to `root`'s markup. */
export class CouponsTab {
	readonly #ui: Controls;
	#projects: Project[] = [];

	constructor(root: HTMLElement) {
		this.#ui = controls(root);
		this.#ui.form.addEventListener("change", () => sync(this.#ui));
		this.#ui.form.addEventListener("submit", (event) => {
			event.preventDefault();
			void this.#submit();
		});
		for (
			const button of root.querySelectorAll<HTMLButtonElement>("[data-preset]")
		) {
			button.addEventListener(
				"click",
				() => applyPreset(this.#ui, Number(button.dataset["preset"])),
			);
		}
		sync(this.#ui);
	}

	/** Offer `projects` as private coupons' owners. */
	setProjects(projects: Project[]): void {
		this.#projects = projects;
		const current = this.#ui.project.value;
		this.#ui.project.replaceChildren(
			h("option", { value: "" }, "Pick a project"),
			...projects.map((project) =>
				h("option", { value: project.id }, `${project.name} (${project.id})`)
			),
		);
		this.#ui.project.value = current;
	}

	/** Start a private coupon for `project`. */
	giveTo(project: Project): void {
		setChoice(this.#ui.form, "visibility", "private");
		this.#ui.project.value = project.id;
		sync(this.#ui);
		this.#ui.name.focus();
	}

	async load(): Promise<void> {
		try {
			this.#render(await api.coupons());
		} catch (error) {
			this.#ui.list.replaceChildren(
				h("li", { class: "empty" }, "No coupons to show."),
			);
			this.#showError(error);
		}
	}

	#render(coupons: Coupon[]): void {
		const names = new Map(
			this.#projects.map((project) => [project.id, project.name]),
		);
		this.#ui.list.replaceChildren(
			...(coupons.length === 0
				? [
					h(
						"li",
						{ class: "empty" },
						"No coupons yet. Make one with the form.",
					),
				]
				: coupons.map((coupon) =>
					couponItem(coupon, names, (target) => void this.#remove(target))
				)),
		);
	}

	async #submit(): Promise<void> {
		const coupon = read(this.#ui);
		if (typeof coupon === "string") {
			announce(this.#ui.status, coupon, "error");
			return;
		}
		const submit = need<HTMLButtonElement>(
			this.#ui.form,
			"button[type=submit]",
		);
		submit.disabled = true;
		try {
			const created = await api.createCoupon(coupon);
			const codes = created.coupon.codes.map((code) => code.code).join(", ");
			announce(
				this.#ui.status,
				created.applied
					? `Made ${codes} and applied it to the project's subscription.`
					: `Made ${codes}. ${
						coupon.visibility === "private"
							? "Only that project can redeem it."
							: "Anyone can enter it at checkout."
					}`,
			);
			this.#ui.form.reset();
			sync(this.#ui);
			await this.load();
		} catch (error) {
			this.#showError(error);
		} finally {
			submit.disabled = false;
		}
	}

	async #remove(coupon: Coupon): Promise<void> {
		try {
			await api.deleteCoupon(coupon.id);
			announce(
				this.#ui.status,
				`Deleted ${
					coupon.name ?? coupon.id
				}. Subscriptions already using it keep it until it ends.`,
			);
			await this.load();
		} catch (error) {
			this.#showError(error);
		}
	}

	#showError(error: unknown): void {
		const message =
			error instanceof api.ApiError && error.code === "billing_not_configured"
				? "Stripe is not configured on this server, so there are no coupons to manage."
				: error instanceof Error
				? error.message
				: "Something went wrong.";
		announce(this.#ui.status, message, "error");
	}
}
