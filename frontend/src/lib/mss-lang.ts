import { LRLanguage, LanguageSupport, indentNodeProp, foldNodeProp, foldInside, delimitedIndent } from "@codemirror/language"
import { styleTags, tags as t } from "@lezer/highlight"

// Minimal MSS language definition for CodeMirror 6
// Note: For a production app, we would use a proper Lezer grammar file (.grammar).
// For this task, we can use a simpler approach or define a basic LRLanguage if possible.
// Since we don't have lezer-generator easily available in this sandbox to build a .grammar,
// we will provide a basic highlighting setup using StreamLanguage if LRLanguage is too complex without a generator.

import { StreamLanguage } from "@codemirror/language"

export const mssLanguage = StreamLanguage.define({
  name: "mss",
  token(stream) {
    if (stream.eatSpace()) return null
    if (stream.match(/^#.*/)) return "comment"
    if (stream.match(/^if\b/) || stream.match(/^else\b/) || stream.match(/^while\b/) || stream.match(/^for\b/) || stream.match(/^in\b/)) return "keyword"
    if (stream.match(/^\$[a-zA-Z_][a-zA-Z0-9_]*/)) return "variableName"
    if (stream.match(/^@[a-zA-Z_][a-zA-Z0-9_]*/)) return "macroName"
    if (stream.match(/^"[^"]*"/)) return "string"
    if (stream.match(/^`[^`]*`/)) return "meta"
    if (stream.match(/^[0-9]+(\.[0-9]+)?/)) return "number"
    if (stream.match(/^[{}()]/)) return "punctuation"
    if (stream.match(/^[=+\-*/<>!]+/)) return "operator"
    if (stream.match(/^[a-zA-Z_][a-zA-Z0-9_]*/)) return "functionName"
    stream.next()
    return null
  }
})

export function mss() {
  return new LanguageSupport(mssLanguage)
}
