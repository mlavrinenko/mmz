// Prose helpers with no dependency on anything generated, so both render paths
// can import them: the docs site from www/utils, docs/src through its lib.typ.

// Spell a small derived count as an English word, so prose that used to read
// "Two gates" does not regress to "2 gates" the moment the number stops being
// hand-typed. Above a table, fall back to the numeral — that is where a numeral
// is what a reader wants anyway.
#let num-word(n) = {
  let words = (
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
  )
  if n >= 0 and n < words.len() { words.at(n) } else { str(n) }
}
