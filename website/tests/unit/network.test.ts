import { assertEquals } from "@std/assert";
import { riveWorthLoading } from "../../src/scripts/network.ts";

Deno.test("Rive loads where the browser reports nothing about the connection", () => {
	assertEquals(riveWorthLoading({}), true);
});

Deno.test("Rive loads on a fast connection with enough memory", () => {
	assertEquals(
		riveWorthLoading({
			connection: { saveData: false, effectiveType: "4g" },
			deviceMemory: 8,
		}),
		true,
	);
});

Deno.test("Rive stays away when the reader saves data", () => {
	assertEquals(
		riveWorthLoading({ connection: { saveData: true, effectiveType: "4g" } }),
		false,
	);
});

Deno.test("Rive stays away on links slower than 4G", () => {
	for (const effectiveType of ["slow-2g", "2g", "3g"]) {
		assertEquals(riveWorthLoading({ connection: { effectiveType } }), false);
	}
});

Deno.test("Rive stays away on devices with under 4 GB of memory", () => {
	assertEquals(riveWorthLoading({ deviceMemory: 2 }), false);
	assertEquals(riveWorthLoading({ deviceMemory: 4 }), true);
});
