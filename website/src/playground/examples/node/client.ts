const input = document.querySelector<HTMLTextAreaElement>("#markdown")!;
const format = document.querySelector<HTMLSelectElement>("#format")!;
const output = document.querySelector<HTMLElement>("#output")!;

// `fetch` to localhost reaches the server on the left.
async function convert() {
	const response = await fetch(
		`http://localhost:3000/convert?to=${format.value}`,
		{
			method: "POST",
			body: input.value,
		},
	);
	output.textContent = await response.text();
}

input.addEventListener("input", convert);
format.addEventListener("change", convert);
await convert();
