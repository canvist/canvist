/**
 * The p95 keystroke-to-paint budget, in milliseconds, measured on Apple
 * silicon. `CANVIST_PERF_SLACK` multiplies it on slower shared runners, the
 * same way the other performance tests do. Linux WebKit CI sets it to 4:
 * those runs land between 10 ms and 26 ms, over the 8 ms Mac budget.
 */
export const KEYSTROKE_P95_BUDGET_MS = 8;

/** The budget for this run, after `CANVIST_PERF_SLACK`. */
export function latencyBudgetMs(
	slack: string | undefined = process.env.CANVIST_PERF_SLACK,
): number {
	const parsed = Number(slack ?? "1");
	const factor = Number.isFinite(parsed) && parsed > 0 ? parsed : 1;

	return KEYSTROKE_P95_BUDGET_MS * factor;
}
