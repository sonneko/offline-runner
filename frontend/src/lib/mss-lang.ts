import { StreamLanguage } from "@codemirror/language";

export const mssLanguage = StreamLanguage.define({
  name: "mss",
  token(stream) {
    if (stream.eatSpace()) return null;
    if (stream.match("//")) {
      stream.skipToEnd();
      return "comment";
    }
    if (stream.match(/^if\b/) || stream.match(/^else\b/) || stream.match(/^for\b/) || stream.match(/^while\b/) || stream.match(/^in\b/)) {
      return "keyword";
    }
    if (stream.match(/^\$[a-zA-Z_][a-zA-Z0-9_]*/)) {
      return "variableName";
    }
    if (stream.match(/^@[a-zA-Z_][a-zA-Z0-9_]*/)) {
      return "macroName";
    }
    if (stream.match(/"/)) {
      while (stream.next() !== '"' && !stream.eol());
      return "string";
    }
    if (stream.match(/`/)) {
      while (stream.next() !== '`' && !stream.eol());
      return "string";
    }
    if (stream.match(/^[0-9]+(\.[0-9]+)?/)) {
      return "number";
    }
    if (stream.match(/^[==|!=|<=|>=|<|>|\+|\-|=]/)) {
      return "operator";
    }
    stream.next();
    return null;
  }
});
