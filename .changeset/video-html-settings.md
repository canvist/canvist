---
"rust:canvist_extension_clipboard": fix
"rust:canvist_headless": fix
"@canvist/headless": fix
---

# Keep video settings through HTML

HTML wrote only a video's source and size, so exporting and reading back a document, or copying and pasting a video, lost a YouTube video's `start` and a Twitch video's `autoplay` and `muted`. The frame now carries `start="30"` as TipTap writes it, and `autoplay` and `muted` as boolean attributes, as `<audio>` does, and the reader takes them back. A video that starts from the beginning writes no `start`.
