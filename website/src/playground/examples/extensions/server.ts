import { allExtensions, createEngine } from "@canvist/headless/full";

// Every extension canvist ships. Switch some off on the left, and their
// syntax below turns back into plain text in the preview, which is the
// editor from @canvist/web/full: type in it.
const engine = await createEngine([...allExtensions]);

// The preview loads nothing from the network, so the image is inline.
const image = `data:image/svg+xml,${
	encodeURIComponent(
		'<svg xmlns="http://www.w3.org/2000/svg" width="160" height="56"><rect width="160" height="56" rx="14" fill="#3B2CF5"/><rect x="77" y="12" width="6" height="32" fill="#F5F3FF"/></svg>',
	)
}`;

const doc = engine.parse(
	"markdown",
	`# Every extension, one document

Switch an extension off and its syntax arrives as **plain text**: ==highlights==, H<sub>2</sub>O, E=mc<sup>2</sup>, and the area $\\pi r^2$.

> [!NOTE]
> Callouts are quotes with a type.

| Extension | Adds |
| --- | --- |
| table | this table |
| highlight | ==marks== |

- [x] Task lists
- [ ] ~~Struck~~ tasks

![A caret](${image})
`,
);

const commands = engine.extensions.flatMap((extension) => extension.commands);
console.log(
	`${engine.extensions.length} extensions bring ${commands.length} commands.`,
);
console.log(doc.export("markdown").split("\n")[0]);
