
/*
  Tel language definition for highlight.js (mdBook bundles hljs 10.1.1).

  This mirrors the JetBrains plugin lexicon so the book and the IDE highlight
  Tel the same way. Keep the keyword / type / constant lists in sync with
  ../../tel-jetbrains/src/main/kotlin/nl/markv/tel/idea/TelLexicon.kt
  (which itself tracks tel/common/src/identifier.rs).

  This file is appended to a copy of the default mdBook highlight.js to form
  theme/highlight.js. Do not edit theme/highlight.js by hand; edit this file
  and re-run theme/build-highlight.sh.
*/
hljs.registerLanguage("tel", function (hljs) {
  var KEYWORDS = {
    keyword:
      "fn struct enum record union data type trait impl " +
      "let uniq const mut local outer " +
      "if elif else then match for in while loop break continue return " +
      "and or not is as " +
      "module use import depends pub " +
      "with new where test req abort todo " +
      "async await yield lazy derive super default static",
    literal: "true false self it none null"
  };

  // Reserved words must never be picked up as a function call (that would
  // shadow keyword colouring for e.g. `if (`, `while (`, `return(`).
  var RESERVED =
    "fn|struct|enum|record|union|data|type|trait|impl|" +
    "let|uniq|const|mut|local|outer|" +
    "if|elif|else|then|match|for|in|while|loop|break|continue|return|" +
    "and|or|not|is|as|" +
    "module|use|import|depends|pub|" +
    "with|new|where|test|req|abort|todo|" +
    "async|await|yield|lazy|derive|super|default|static|" +
    "true|false|self|it|none|null";

  // `#` line comment and `##` doc comment both run to end of line.
  var COMMENT = hljs.COMMENT(/#/, /$/);

  var NUMBER = {
    className: "number",
    begin: /\b\d[\d_]*(?:\.\d[\d_]*)?\b/,
    relevance: 0
  };

  var STRING = {
    className: "string",
    variants: [
      { begin: '"""', end: '"""', contains: [hljs.BACKSLASH_ESCAPE] },
      { begin: '"', end: '"', illegal: /\n/, contains: [hljs.BACKSLASH_ESCAPE] },
      { begin: "'", end: "'", illegal: /\n/, contains: [hljs.BACKSLASH_ESCAPE] },
      { begin: "`", end: "`" } // tagged DSL literal, may span lines
    ]
  };

  // Capitalised identifiers are types: covers both built-ins (Int64, Text, …)
  // and user-defined types (Order, …), matching the IDE's USER_TYPE rule.
  var TYPE = {
    className: "type",
    begin: /\b[A-Z][A-Za-z0-9_]*\b/,
    relevance: 0
  };

  // A lower-case identifier directly followed by `(` is a function/method call.
  var FUNCTION_CALL = {
    className: "title",
    begin: new RegExp(
      "\\b(?!(?:" + RESERVED + ")\\b)[a-z_][A-Za-z0-9_]*(?=\\s*\\()"
    ),
    relevance: 0
  };

  return {
    name: "Tel",
    aliases: ["tel"],
    case_insensitive: false,
    keywords: KEYWORDS,
    contains: [
      COMMENT,
      STRING,
      NUMBER,
      FUNCTION_CALL,
      TYPE
    ]
  };
});
