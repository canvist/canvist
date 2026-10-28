import { createEngine } from "@canvist/headless";
import { createServer } from "node:http";

// A Node server, as you'd write it for `node server.ts`: `node:http`
// answers POST /convert by converting the body with canvist.
const engine = await createEngine();
const formats = ["json", "html", "markdown", "text"] as const;
type Format = (typeof formats)[number];

const server = createServer(async (request, response) => {
	const url = new URL(request.url ?? "/", "http://localhost");
	const to = url.searchParams.get("to") ?? "html";
	if (
		request.method !== "POST" || url.pathname !== "/convert" ||
		!formats.includes(to as Format)
	) {
		response.writeHead(404, { "content-type": "text/plain" }).end(
			"Try POST /convert?to=html",
		);
		return;
	}

	let body = "";
	for await (const chunk of request) body += chunk;
	const doc = engine.parse("markdown", body);
	console.log(`${request.method} ${request.url}: ${body.length} characters in`);
	response.writeHead(200, { "content-type": "text/plain; charset=utf-8" });
	response.end(doc.export(to as Format));
});

server.listen(
	3000,
	() => console.log("Converting on http://localhost:3000/convert"),
);
