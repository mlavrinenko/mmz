//! Canonical rendering of a matched node — or, under `capture:`, of the parts
//! of it the pattern named: what an `ast:` probe actually hashes.
//!
//! # Why a rendering and not the matched text
//!
//! This is the same decision [`crate::json`] makes when it sorts object keys on
//! the way out, for the same reason. Hashing the matched *source text* would
//! make whitespace an input: a `cargo fmt` that reflows a signature across two
//! lines would report a changed public API, and the rule would re-run against a
//! file whose meaning did not move. Rendering the tree instead drops every byte
//! the parser dropped — indentation, line breaks, the spaces around `->` — and
//! keeps every byte it kept.
//!
//! # What it keeps, and why that is all of it
//!
//! [`render`] walks *every* child, named and anonymous, and prints each leaf's
//! exact text. Anonymous children are the operators and keywords, so `a + b`
//! and `a - b` render differently — a named-children-only walk (tree-sitter's
//! own `to_sexp`) collapses them, and a digest that cannot tell `+` from `-` is
//! precisely the lie this tool exists to refuse. Leaf text is exact rather than
//! whitespace-normalised, so `"a   b"` and `"a b"` stay distinct string
//! literals.
//!
//! Between them those two rules make the rendering a lossless encoding of the
//! match's token sequence and tree shape. Nothing that changes the code can
//! leave the digest alone; only the inter-token whitespace, which no parse
//! records, is gone.
//!
//! # The cost this carries
//!
//! A rendering names node *kinds*, so it is pinned to the grammar that produced
//! them: upgrading mmz to a build with a newer tree-sitter grammar can change a
//! kind name and move every digest that mentions it. That is a false stale —
//! rules re-run once and settle — and it is the direction to fail in. Hashing
//! the matched text instead would be stable across grammar bumps and unstable
//! across reformatting, which trades a false stale for a *missed* one.

use ast_grep_core::Node;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::tree_sitter::StrDoc;
use ast_grep_language::SupportLang;

/// The document type every `ast:` probe parses into.
pub(crate) type Doc = StrDoc<SupportLang>;

/// One step of the explicit walk in [`render`].
enum Step<'r> {
    /// Render this node and queue its children.
    Open(Node<'r, Doc>),
    /// Close the parenthesis opened for a node whose children are done.
    Close,
}

/// One matched node as canonical bytes.
///
/// The shape is an s-expression — `(kind child …)` for a node with children,
/// `(kind "text")` for a leaf — so a reader can eyeball what a probe measured
/// and see the tree rather than a hash.
///
/// The walk is an explicit stack rather than recursion because tree depth is
/// the input's to choose: a generated or minified source can nest thousands of
/// levels, and a recursive renderer would meet it with a stack overflow, which
/// aborts the process instead of failing the probe.
pub(crate) fn render(node: &Node<'_, Doc>) -> Vec<u8> {
    let mut out = String::new();
    render_into(&mut out, node);
    out.into_bytes()
}

/// [`render`]'s walk, writing into a buffer a caller may already be building.
/// [`render_captures`] needs exactly that, and going through `Vec<u8>` to get
/// there would re-validate bytes this module only ever wrote as UTF-8.
fn render_into(out: &mut String, node: &Node<'_, Doc>) {
    let mut stack = vec![Step::Open(node.clone())];
    while let Some(step) = stack.pop() {
        let Step::Open(node) = step else {
            out.push(')');
            continue;
        };
        separate(out);
        out.push('(');
        out.push_str(&node.kind());
        if node.is_leaf() {
            out.push(' ');
            // `str`'s Debug is a deterministic, injective escaping — distinct
            // strings never render alike — so a leaf's text can carry a quote,
            // a backslash or a newline without colliding with the syntax.
            out.push_str(&format!("{:?}", &*node.text()));
            out.push(')');
            continue;
        }
        stack.push(Step::Close);
        let children: Vec<Node<'_, Doc>> = node.children().collect();
        for child in children.into_iter().rev() {
            stack.push(Step::Open(child));
        }
    }
}

/// Puts a space before a node unless it opens the rendering or its parent, so
/// `(a(b))` never happens and `(a (b) (c))` reads as the tree it is.
fn separate(out: &mut String) {
    if !out.is_empty() && !out.ends_with('(') {
        out.push(' ');
    }
}

/// One match's *captures* as canonical bytes: the shape a probe hashes when it
/// declares `capture:`.
///
/// Each named metavariable renders as `($NAME …)` wrapping the rendering of
/// every node bound to it, so the reading is the same s-expression as
/// [`render`] with the pattern's own names standing where the matched node
/// would have. The `$` is not decoration — it is what keeps a capture wrapper
/// from ever reading as a node of some grammar whose kinds happen to be
/// uppercase.
///
/// `names` arrives sorted and deduplicated (see [`crate::ast::select`]), which
/// is what makes reordering a `capture:` list not an edit to the input: the
/// list is the *set* of parts that matter, and the order it was typed in is the
/// author's presentation rather than anything the document said. That is the
/// same call [`crate::json`] makes on object keys, and the reason it is safe
/// here and not for match order is that no two capture sets differing in
/// content can normalise together — only two spellings of one set can.
///
/// A single capture (`$NAME`) binds one node, a multi capture (`$$$ARGS`) binds
/// a run of them, and one that bound none renders as a bare `($ARGS)` — which
/// is distinct from every count above it, so "the argument list emptied" stays
/// an edit. A name the pattern uses twice binds once: ast-grep keys its env by
/// name and requires the two occurrences to have matched identical nodes, so
/// there is nothing for a second mention to add.
pub(crate) fn render_captures(env: &MetaVarEnv<'_, Doc>, names: &[&str]) -> Vec<u8> {
    let mut out = String::new();
    for name in names {
        separate(&mut out);
        out.push_str("($");
        out.push_str(name);
        for node in bound(env, name) {
            render_into(&mut out, &node);
        }
        out.push(')');
    }
    out.into_bytes()
}

/// The nodes `name` is bound to in this match: one for a single capture, a run
/// for a multi capture, none for a multi capture that matched nothing.
///
/// The two are asked for in that order rather than distinguished up front
/// because ast-grep stores them in separate maps and a name is only ever in
/// one: a pattern cannot spell `$NAME` and `$$$NAME` as the same variable.
fn bound<'t>(env: &MetaVarEnv<'t, Doc>, name: &str) -> Vec<Node<'t, Doc>> {
    match env.get_match(name) {
        Some(node) => vec![node.clone()],
        None => env.get_multiple_matches(name),
    }
}
