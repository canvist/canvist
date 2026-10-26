/**
 * Execute the workflow's provenance gate against real local Git refs, without
 * publication credentials or registry access. Run from the repository root:
 * deno test --allow-read --allow-write --allow-run=git,bash tools/release/publish_provenance_test.ts
 */
import { assert, assertEquals, assertStringIncludes } from "jsr:@std/assert@^1";
import { parse } from "jsr:@std/yaml@^1";

const ROOT = new URL("../../", import.meta.url).pathname;
const workflow: unknown = parse(
	await Deno.readTextFile(`${ROOT}.github/workflows/publish.yml`),
);

/** Read YAML fields while keeping parsed data unknown until checked. */
function field(value: unknown, ...keys: string[]): unknown {
	for (const key of keys) {
		assert(value !== null && typeof value === "object" && key in value, key);
		value = Reflect.get(value, key);
	}

	return value;
}

/** Locate a workflow step so tests execute the exact commands shipped to CI. */
function step(job: string, name: string): unknown {
	const steps = field(workflow, "jobs", job, "steps");
	assert(Array.isArray(steps));
	const result: unknown = steps.find((item: unknown) =>
		field(item, "name") === name
	);
	assert(result !== undefined, `Missing ${job} step: ${name}`);

	return result;
}

/** Run a Git command or workflow block and retain failures for assertions. */
async function run(
	command: string,
	args: string[],
	cwd: string,
	env: Record<string, string> = {},
) {
	const result = await new Deno.Command(command, {
		args,
		cwd,
		env,
		stdout: "piped",
		stderr: "piped",
	}).output();
	const decoder = new TextDecoder();

	return {
		success: result.success,
		stdout: decoder.decode(result.stdout).trim(),
		stderr: decoder.decode(result.stderr).trim(),
	};
}

/** Require fixture setup to succeed rather than obscuring a provenance failure. */
async function git(cwd: string, ...args: string[]): Promise<string> {
	const result = await run("git", args, cwd);
	assert(result.success, result.stderr);

	return result.stdout;
}

/** Supply dispatch inputs as environment values, exactly as the workflow does. */
async function plan(cwd: string, name: string, env: Record<string, string>) {
	const script = field(step("plan", name), "run");
	assert(typeof script === "string");
	await Deno.writeTextFile(`${cwd}/output`, "");
	const result = await run("bash", ["-c", script], cwd, {
		DRY_RUN: "false",
		CHECKOUT_REF: "",
		RELEASE_TAG: "v-test",
		GITHUB_OUTPUT: `${cwd}/output`,
		...env,
	});

	return { ...result, output: await Deno.readTextFile(`${cwd}/output`) };
}

Deno.test("real checkout overrides fail before any checkout or build", async () => {
	const cwd = await Deno.makeTempDir();

	try {
		const result = await plan(cwd, "choose jobs for the tag", {
			CHECKOUT_REF: "attacker-branch",
		});
		assert(!result.success, "Real publication accepted checkout_ref");
		assertStringIncludes(result.stderr, "checkout_ref");
	} finally {
		await Deno.remove(cwd, { recursive: true });
	}
});

Deno.test("dry runs retain checkout overrides and pull-request planning", async () => {
	const cwd = await Deno.makeTempDir();

	try {
		const result = await plan(cwd, "choose jobs for the tag", {
			DRY_RUN: "true",
			CHECKOUT_REF: "release-branch",
			RELEASE_TAG: "pull-request",
		});
		assert(result.success, result.stderr);
		assertStringIncludes(result.output, "sdk=true");
		assertStringIncludes(result.output, "app=true");
	} finally {
		await Deno.remove(cwd, { recursive: true });
	}
});

Deno.test("the plan binds publication to the tag's commit on main", async (t) => {
	const cwd = await Deno.makeTempDir();

	try {
		// Reuse existing objects: no test commits, credentials, or network needed.
		await git(
			cwd,
			"clone",
			"--quiet",
			"--bare",
			"--shared",
			ROOT,
			"remote.git",
		);
		const tip = await git(ROOT, "rev-parse", "HEAD");
		const main = await git(ROOT, "rev-parse", "HEAD~1");
		const release = await git(ROOT, "rev-parse", "HEAD~2");
		await git(
			cwd,
			"--git-dir=remote.git",
			"update-ref",
			"refs/heads/main",
			main,
		);
		await git(
			cwd,
			"--git-dir=remote.git",
			"update-ref",
			"refs/tags/v-test",
			release,
		);
		await git(cwd, "clone", "--quiet", "--shared", "remote.git", "checkout");
		const checkout = `${cwd}/checkout`;

		await t.step(
			"accepts a tag on main and emits its immutable SHA",
			async () => {
				await git(checkout, "checkout", "--quiet", "--detach", release);
				const result = await plan(
					checkout,
					"resolve and verify the release commit",
					{},
				);
				assert(result.success, result.stderr);
				assertEquals(result.output, `sha=${release}\n`);
			},
		);

		await t.step("rejects a checkout that differs from the tag", async () => {
			await git(checkout, "checkout", "--quiet", "--detach", main);
			const result = await plan(
				checkout,
				"resolve and verify the release commit",
				{},
			);
			assert(!result.success);
			assertEquals(result.output, "");
		});

		await t.step("rejects a tag outside main", async () => {
			await git(
				cwd,
				"--git-dir=remote.git",
				"update-ref",
				"refs/tags/v-test",
				tip,
			);
			await git(checkout, "checkout", "--quiet", "--detach", tip);
			const result = await plan(
				checkout,
				"resolve and verify the release commit",
				{},
			);
			assert(!result.success);
			assertEquals(result.output, "");
		});

		await t.step("rejects a missing tag", async () => {
			const result = await plan(
				checkout,
				"resolve and verify the release commit",
				{
					RELEASE_TAG: "v-missing",
				},
			);
			assert(!result.success);
			assertEquals(result.output, "");
		});

		await t.step(
			"pins a dry-run branch without requiring a release tag",
			async () => {
				const result = await plan(
					checkout,
					"resolve and verify the release commit",
					{
						DRY_RUN: "true",
						RELEASE_TAG: "pull-request",
					},
				);
				assert(result.success, result.stderr);
				assertEquals(result.output, `sha=${tip}\n`);
			},
		);
	} finally {
		await Deno.remove(cwd, { recursive: true });
	}
});

Deno.test("every build consumes the plan SHA without persisting credentials", () => {
	assertEquals(
		field(workflow, "jobs", "plan", "outputs", "sha"),
		"${{ steps.source.outputs.sha }}",
	);
	assertEquals(
		field(step("plan", "checkout source"), "with", "persist-credentials"),
		false,
	);

	for (const job of ["packages", "apple", "android", "pub", "deploy"]) {
		const needs = field(workflow, "jobs", job, "needs");
		assert(
			needs === "plan" || (Array.isArray(needs) && needs.includes("plan")),
			job,
		);
		const checkout = field(step(job, "checkout release"), "with");
		assertEquals(field(checkout, "ref"), "${{ needs.plan.outputs.sha }}", job);
		assertEquals(field(checkout, "persist-credentials"), false, job);
	}

	assertEquals(
		field(workflow, "jobs", "apple", "permissions", "contents"),
		"read",
	);
	assertEquals(
		field(workflow, "jobs", "apple-release", "environment"),
		"publisher",
	);
	assertEquals(field(workflow, "jobs", "apple-release", "needs"), [
		"plan",
		"apple",
	]);
	assertEquals(
		field(workflow, "jobs", "apple-release", "if"),
		"needs.plan.outputs.dry_run != 'true'",
	);
	const releaseCondition = field(workflow, "jobs", "release", "if");
	assert(typeof releaseCondition === "string");
	assertStringIncludes(releaseCondition, "needs.plan.result == 'success'");
});
