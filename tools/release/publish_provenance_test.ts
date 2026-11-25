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

	for (const job of ["crates", "npm", "apple", "android", "pub", "deploy"]) {
		const needs = field(workflow, "jobs", job, "needs");
		assert(
			needs === "plan" || (Array.isArray(needs) && needs.includes("plan")),
			job,
		);
		const checkout = field(step(job, "checkout release"), "with");
		assertEquals(field(checkout, "ref"), "${{ needs.plan.outputs.sha }}", job);
		assertEquals(field(checkout, "persist-credentials"), false, job);
	}

	// pub.dev pins crates.io, not npm — it must not wait on the wasm path.
	assertEquals(field(workflow, "jobs", "pub", "needs"), ["plan", "crates"]);
	assertEquals(
		field(workflow, "jobs", "release", "needs"),
		[
			"plan",
			"configuration",
			"crates",
			"npm",
			"apple",
			"apple-release",
			"android",
			"pub",
			"deploy",
		],
	);

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

Deno.test("missing native credentials stop publication before any packages ship", async () => {
	const script = field(
		step("configuration", "require every native publishing credential"),
		"run",
	);
	assert(typeof script === "string");
	const names = [
		"MAVEN_CENTRAL_USERNAME",
		"MAVEN_CENTRAL_PASSWORD",
		"MAVEN_SIGNING_KEY",
		"MAVEN_SIGNING_KEY_PASSWORD",
		"SWIFTPM_GITHUB_TOKEN",
	];
	const env = Object.fromEntries(names.map((name) => [name, "fixture"]));
	const ready = await run("bash", ["-c", script], ROOT, env);
	assert(ready.success, ready.stderr);
	for (const name of names) {
		const missing = await run("bash", ["-c", script], ROOT, {
			...env,
			[name]: "",
		});
		assert(!missing.success, `${name} must be required`);
		assertStringIncludes(missing.stderr, name);
	}
	for (const job of ["crates", "npm", "apple", "android"]) {
		const needs = field(workflow, "jobs", job, "needs");
		assert(Array.isArray(needs) && needs.includes("configuration"));
	}
});

/** Create an executable stub binary in a temporary directory for offline step tests. */
async function writeStub(
	dir: string,
	name: string,
	body: string,
): Promise<void> {
	const path = `${dir}/${name}`;
	await Deno.writeTextFile(
		path,
		`#!/usr/bin/env bash\nset -euo pipefail\n${body}\n`,
	);
	await Deno.chmod(path, 0o755);
}

/** Read PATH via bash so tests work without Deno's `--allow-env` flag (matching CI). */
async function hostPath(): Promise<string> {
	const result = await run("bash", ["-c", 'printf "%s" "$PATH"'], ROOT);
	assert(result.success, result.stderr);
	return result.stdout || "/usr/bin:/bin";
}

/** Build a clean environment stripped of host npm/OIDC variables for deterministic step tests. */
function cleanStepEnv(
	basePath: string,
	stubBin: string,
	extra: Record<string, string>,
): Record<string, string> {
	return {
		HOME: stubBin,
		PATH: `${stubBin}:${basePath}`,
		...extra,
	};
}

/** Run a workflow step script with an isolated environment so host tokens never leak into tests. */
async function runIsolated(
	command: string,
	args: string[],
	cwd: string,
	env: Record<string, string>,
) {
	const result = await new Deno.Command(command, {
		args,
		cwd,
		env,
		clearEnv: true,
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

Deno.test("npm trusted publishing preflight validates versions, auth hygiene, OIDC env, repo URL, and 48h expiry risk", async (t) => {
	const script = field(
		step("npm", "verify npm trusted publishing setup"),
		"run",
	);
	assert(typeof script === "string");

	assertEquals(
		field(step("npm", "install node"), "with", "package-manager-cache"),
		false,
	);
	assertEquals(
		field(step("npm", "publish npm packages"), "env", "NPM_CONFIG_LOGLEVEL"),
		"verbose",
	);
	assertEquals(
		field(step("npm", "keep npm debug logs on failure"), "if"),
		"failure()",
	);

	const cwd = await Deno.makeTempDir();
	const bin = `${cwd}/bin`;
	const repo = `${cwd}/repo`;
	await Deno.mkdir(bin, { recursive: true });
	await Deno.mkdir(`${repo}/packages/canvist-core`, { recursive: true });
	await Deno.mkdir(`${repo}/packages/canvist-headless`, { recursive: true });

	try {
		await Deno.writeTextFile(
			`${repo}/monochange.toml`,
			[
				`[package."@canvist/core"]`,
				`path = "packages/canvist-core"`,
				`[package."@canvist/headless"]`,
				`path = "packages/canvist-headless"`,
			].join("\n"),
		);
		await Deno.writeTextFile(
			`${repo}/packages/canvist-core/package.json`,
			JSON.stringify({
				name: "@canvist/core",
				repository: {
					type: "git",
					url: "git+https://github.com/canvist/canvist.git",
				},
			}),
		);
		await Deno.writeTextFile(
			`${repo}/packages/canvist-headless/package.json`,
			JSON.stringify({
				name: "@canvist/headless",
				repository: {
					type: "git",
					url: "git+https://github.com/canvist/canvist.git",
				},
			}),
		);
		const readinessPath = `${cwd}/publish-readiness.json`;
		await Deno.writeTextFile(
			readinessPath,
			JSON.stringify({
				packages: [
					{
						package: "@canvist/core",
						registry: "npm",
						status: "ready",
					},
					{
						package: "@canvist/headless",
						registry: "npm",
						status: "ready",
					},
				],
			}),
		);

		await writeStub(bin, "node", `echo "\${STUB_NODE_VERSION:-v24.22.3}"`);
		await writeStub(
			bin,
			"npm",
			`case "$*" in
  "--version") echo "\${STUB_NPM_VERSION:-11.19.1}" ;;
  "config get registry") echo "\${STUB_NPM_REGISTRY:-https://registry.npmjs.org/}" ;;
  "config list") printf '%s\\n' "\${STUB_NPM_CONFIG:-registry = \\"https://registry.npmjs.org/\\"}" ;;
  *) echo "unexpected npm invocation: $*" >&2; exit 2 ;;
esac`,
		);
		await writeStub(
			bin,
			"curl",
			`url="\${!#}"
case "$url" in
  */@canvist/core)
    echo '{"time":{"created":"2020-01-01T00:00:00.000Z"},"versions":{"0.0.0":{"_npmUser":{"name":"person"}}}}'
    ;;
  */@canvist/headless)
    echo '{"time":{"created":"2020-01-01T00:00:00.000Z"},"versions":{"0.2.1":{"_npmUser":{"trustedPublisher":{"id":"github"}}}}}'
    ;;
  *) exit 22 ;;
esac`,
		);

		const basePath = await hostPath();
		const okEnv = cleanStepEnv(basePath, bin, {
			REQUIRE_OIDC: "true",
			READINESS_REPORT: readinessPath,
			GITHUB_ACTIONS: "true",
			ACTIONS_ID_TOKEN_REQUEST_URL: "https://oidc.example/token",
			ACTIONS_ID_TOKEN_REQUEST_TOKEN: "fixture-request-token",
		});

		await t.step(
			"passes cleanly and warns only on >48h unvalidated packages",
			async () => {
				const result = await runIsolated("bash", ["-c", script], repo, okEnv);
				assert(result.success, result.stderr);
				assertStringIncludes(
					result.stdout,
					"::warning::1 ready npm package(s)",
				);
				assertStringIncludes(result.stdout, "@canvist/core");
				assert(!result.stdout.includes("@canvist/headless"));
			},
		);

		await t.step("rejects outdated Node and npm versions", async () => {
			const oldNode = await runIsolated("bash", ["-c", script], repo, {
				...okEnv,
				STUB_NODE_VERSION: "v22.13.9",
			});
			assert(!oldNode.success);
			assertStringIncludes(oldNode.stderr, "Node >= 22.14.0");

			const oldNpm = await runIsolated("bash", ["-c", script], repo, {
				...okEnv,
				STUB_NPM_VERSION: "11.5.0",
			});
			assert(!oldNpm.success);
			assertStringIncludes(oldNpm.stderr, "npm >= 11.5.1");
		});

		await t.step(
			"rejects static auth env vars and .npmrc credentials",
			async () => {
				for (
					const key of [
						"NODE_AUTH_TOKEN",
						"NPM_TOKEN",
						"NPM_CONFIG_USERCONFIG",
						"npm_config_registry_auth_token",
					]
				) {
					const result = await runIsolated("bash", ["-c", script], repo, {
						...okEnv,
						[key]: "fixture",
					});
					assert(!result.success, `${key} must be rejected`);
					assertStringIncludes(result.stderr, key);
				}

				const authNpmrc = await runIsolated("bash", ["-c", script], repo, {
					...okEnv,
					STUB_NPM_CONFIG: "//registry.npmjs.org/:_authToken = (protected)",
				});
				assert(!authNpmrc.success);
				assertStringIncludes(authNpmrc.stderr, "Static npm credentials");
			},
		);

		await t.step(
			"requires OIDC env vars unless running on a fork PR",
			async () => {
				const missingOidc = await runIsolated("bash", ["-c", script], repo, {
					...okEnv,
					ACTIONS_ID_TOKEN_REQUEST_URL: "",
				});
				assert(!missingOidc.success);
				assertStringIncludes(
					missingOidc.stderr,
					"ACTIONS_ID_TOKEN_REQUEST_URL",
				);

				const forkPr = await runIsolated("bash", ["-c", script], repo, {
					...okEnv,
					REQUIRE_OIDC: "false",
					ACTIONS_ID_TOKEN_REQUEST_URL: "",
					ACTIONS_ID_TOKEN_REQUEST_TOKEN: "",
					SKIP_REGISTRY_CHECK: "true",
				});
				assert(forkPr.success, forkPr.stderr);
			},
		);

		await t.step(
			"rejects mismatched repository.url in package.json",
			async () => {
				await Deno.writeTextFile(
					`${repo}/packages/canvist-core/package.json`,
					JSON.stringify({
						name: "@canvist/core",
						repository: {
							type: "git",
							url: "git+https://github.com/other/repo.git",
						},
					}),
				);
				const result = await runIsolated("bash", ["-c", script], repo, {
					...okEnv,
					SKIP_REGISTRY_CHECK: "true",
				});
				assert(!result.success);
				assertStringIncludes(result.stderr, "repository.url");
			},
		);
	} finally {
		await Deno.remove(cwd, { recursive: true });
	}
});

Deno.test("npm dry-run verification requires OIDC token exchange attempts for planned packages", async (t) => {
	const script = field(
		step("npm", "check the npm dry run covers every package"),
		"run",
	);
	assert(typeof script === "string");

	const cwd = await Deno.makeTempDir();
	try {
		await Deno.writeTextFile(
			`${cwd}/publish-readiness.json`,
			JSON.stringify({
				packages: [{
					package: "@canvist/core",
					registry: "npm",
					status: "ready",
				}],
			}),
		);
		const baseEnv = {
			RUNNER_TEMP: cwd,
			REQUIRE_OIDC: "true",
		};

		await t.step(
			"passes when planned packages record GitHub OIDC fetch and npm exchange",
			async () => {
				await Deno.writeTextFile(
					`${cwd}/publish-packages.json`,
					JSON.stringify({
						package_publish: {
							packages: [{
								package: "@canvist/core",
								status: "planned",
								stderr:
									"npm http fetch GET https://oidc.example/idtoken?api-version=2.0&audience=npm%3Aregistry.npmjs.org 200 120ms\nnpm http fetch POST 404 https://registry.npmjs.org/-/npm/v1/oidc/token/exchange/package/%40canvist%2Fcore 95ms\n",
							}],
							summary: { failed: 0, blocked: 0 },
						},
					}),
				);
				const result = await run("bash", ["-c", script], cwd, baseEnv);
				assert(result.success, result.stderr);
			},
		);

		await t.step(
			"fails when a planned package never attempted OIDC exchange",
			async () => {
				await Deno.writeTextFile(
					`${cwd}/publish-packages.json`,
					JSON.stringify({
						package_publish: {
							packages: [{
								package: "@canvist/core",
								status: "planned",
								stderr:
									"npm warn publish This command requires you to be logged in (dry-run)\n",
							}],
							summary: { failed: 0, blocked: 0 },
						},
					}),
				);
				const result = await run("bash", ["-c", script], cwd, baseEnv);
				assert(!result.success);
				assertStringIncludes(result.stderr, "OIDC token exchange");
				assertStringIncludes(result.stderr, "@canvist/core");
			},
		);
	} finally {
		await Deno.remove(cwd, { recursive: true });
	}
});

Deno.test("npm failure diagnostics extract OIDC lines and strip JWTs", async () => {
	const script = field(
		step("npm", "show npm OIDC diagnostics on failure"),
		"run",
	);
	assert(typeof script === "string");

	const cwd = await Deno.makeTempDir();
	try {
		await Deno.writeTextFile(
			`${cwd}/2026-11-25T00_00_00_000Z-debug-0.log`,
			[
				"1 http fetch POST 404 https://registry.npmjs.org/-/npm/v1/oidc/token/exchange/package/%40canvist%2Fcore 120ms",
				"2 verbose oidc Failed token exchange request with body message: OIDC token exchange error - eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJ0ZXN0In0.sig123",
			].join("\n"),
		);
		const result = await run("bash", ["-c", script], cwd, {
			NPM_CONFIG_LOGS_DIR: cwd,
		});
		assert(result.success, result.stderr);
		assertStringIncludes(result.stdout, "token/exchange");
		assertStringIncludes(result.stdout, "<redacted-jwt>");
		assert(!result.stdout.includes("eyJhbGciOiJSUzI1NiJ9"));
	} finally {
		await Deno.remove(cwd, { recursive: true });
	}
});

Deno.test("pub.dev publishing skips already-published versions on resume while keeping dry runs active", async (t) => {
	const script = field(
		step("pub", "publish canvist, canvist_extensions, then canvist_flutter"),
		"run",
	);
	assert(typeof script === "string");

	const cwd = await Deno.makeTempDir();
	const bin = `${cwd}/bin`;
	const repo = `${cwd}/repo`;
	const logFile = `${cwd}/calls.log`;
	await Deno.mkdir(bin, { recursive: true });
	await Deno.mkdir(`${repo}/platforms/dart`, { recursive: true });
	await Deno.mkdir(`${repo}/platforms/dart_extensions`, { recursive: true });
	await Deno.mkdir(`${repo}/platforms/flutter`, { recursive: true });

	try {
		await writeStub(
			bin,
			"dart",
			`printf 'dart:%s:%s\\n' "$(basename "$PWD")" "$*" >> "${logFile}"`,
		);
		await writeStub(
			bin,
			"flutter",
			`printf 'flutter:%s:%s\\n' "$(basename "$PWD")" "$*" >> "${logFile}"`,
		);
		await writeStub(
			bin,
			"curl",
			`url="\${!#}"
case "$url" in
  */packages/canvist/versions/0.2.2) printf '200' ;;
  */packages/canvist_extensions/versions/0.2.2) printf '404' ;;
  */packages/canvist_flutter/versions/0.2.2) printf '200' ;;
  *) printf '404' ;;
esac`,
		);

		const basePath = await hostPath();

		await t.step("dry runs validate all three packages", async () => {
			await Deno.writeTextFile(logFile, "");
			const result = await runIsolated(
				"bash",
				["-c", script],
				repo,
				cleanStepEnv(basePath, bin, { DRY_RUN: "true", RELEASE_TAG: "v0.2.2" }),
			);
			assert(result.success, result.stderr);
			const calls = (await Deno.readTextFile(logFile)).trim().split("\n");
			assertEquals(calls, [
				"dart:dart:pub publish --dry-run",
				"dart:dart_extensions:pub publish --dry-run",
				"flutter:flutter:pub publish --dry-run",
			]);
		});

		await t.step(
			"real runs skip versions already live on pub.dev and publish missing ones",
			async () => {
				await Deno.writeTextFile(logFile, "");
				const result = await runIsolated(
					"bash",
					["-c", script],
					repo,
					cleanStepEnv(basePath, bin, {
						DRY_RUN: "false",
						RELEASE_TAG: "v0.2.2",
					}),
				);
				assert(result.success, result.stderr);
				assertStringIncludes(
					result.stdout,
					"canvist 0.2.2 already exists on pub.dev",
				);
				assertStringIncludes(
					result.stdout,
					"canvist_flutter 0.2.2 already exists on pub.dev",
				);
				const calls = (await Deno.readTextFile(logFile)).trim().split("\n");
				assertEquals(calls, ["dart:dart_extensions:pub publish --force"]);
			},
		);
	} finally {
		await Deno.remove(cwd, { recursive: true });
	}
});
