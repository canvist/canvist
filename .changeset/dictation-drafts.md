---
"canvist_core": minor
"canvist_layout": minor
"canvist_headless": minor
"canvist_ffi": minor
---

# Dictation: drafts that show faint at the caret and settle into the document

Speech now becomes text the way people expect from live captions: the words still being said appear at the caret, rewrite themselves as the speech model changes its mind, and settle into the document once they're final.

`canvist_core` adds `Editor::dictate`, which takes each segment of speech as a `Transcript::Partial` or `Transcript::Final`. Provisional text never enters the document: it isn't sent to collaborators, saved, exported, or added to history. Final segments are inserted at the caret in segment order, each as its own undo step (`EditKind::Dictation`). Segments join the text around them with one space, except before punctuation and after opening brackets and quotes, and the first transcript replaces a selection as typing does. `EditorState::dictation` reports the draft (`Dictation::provisional`) and the text inserted a moment ago (`Dictation::settling`), mapped through every later change. `Editor::stop_dictation` drops the draft.

`canvist_layout` adds `DocumentLayout::set_provisional`, behind the new `provisional` feature (on by default, so builds that never dictate, such as the browser editor, carry none of it). The block holding the draft is laid out with it inserted, so the text after it reflows as it will once the text is final, while every query (hit testing, carets, selections, line navigation, slots, and line ranges) stays in document offsets. A caret where the draft sits shows after it. The draft is drawn at 45% opacity and breathes gently while the animation clock runs. `DisplayOptions::settling` fades newly final text into ink over 450 ms. Without a clock, as for reduced motion, the draft is steady and final text is solid at once.

`canvist_headless` adds `Doc::stop_dictation`.

`canvist_ffi` adds `Editor.dictate(segment, text, isFinal)`, `stopDictation()`, and `dictationDraft()` for the iOS and Android bindings, `EditorChange.dictation`, and `Editor.setAnimations` with `Frame.animated`, so a view that asks for a frame on each display refresh animates drafts, settling text, and kinetic text. Animations are off until the view turns them on.

Revising a draft relays out its paragraph in about 29 µs in a 10,000-paragraph document, and a native frame after a revision takes about 30 µs.
