/** The projects tab: every project, its plan, and this month's usage. */

import { formatCount, type Project } from "../../lib/admin.ts";
import { h } from "./dom.ts";

const STANDING: Record<Project["standing"], string> = {
	ok: "Within plan",
	approaching: "Near the limit",
	over: "Over, billed",
	capped: "At the free cap",
};

function usage(project: Project): string {
	const included = project.included_documents === null
		? "custom"
		: formatCount(project.included_documents);
	return `${formatCount(project.active_documents)} of ${included}`;
}

function row(project: Project, give: (project: Project) => void): HTMLElement {
	const button = h(
		"button",
		{ type: "button", class: "link-button" },
		"Give a coupon",
	);
	const standing = h(
		"span",
		{ class: "projects__standing", "data-standing": project.standing },
		STANDING[project.standing],
	);
	button.addEventListener("click", () => give(project));

	return h(
		"tr",
		{},
		h(
			"th",
			{ scope: "row" },
			h("span", { class: "projects__name" }, project.name),
			h("span", { class: "projects__id code-label" }, project.id),
		),
		h("td", { class: "projects__plan" }, project.plan),
		h("td", {}, h("span", { class: "code-label" }, usage(project)), standing),
		h("td", {}, project.subscription_status?.replace("_", " ") ?? "none"),
		h("td", {}, button),
	);
}

/** Draw `projects` into `target`; `give` starts a private coupon for one. */
export function renderProjects(
	target: HTMLElement,
	projects: Project[],
	give: (project: Project) => void,
): void {
	if (projects.length === 0) {
		target.replaceChildren(h("p", { class: "empty" }, "No projects yet."));
		return;
	}

	const titles = [
		"Project",
		"Plan",
		"Documents this month",
		"Subscription",
		"",
	];
	const head = h(
		"thead",
		{},
		h("tr", {}, ...titles.map((title) => h("th", { scope: "col" }, title))),
	);
	const body = h("tbody", {}, ...projects.map((project) => row(project, give)));
	target.replaceChildren(h("table", { class: "projects" }, head, body));
}
