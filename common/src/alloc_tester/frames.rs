//! Which frames of a captured stack a reader of a failed allocation test
//! wants, and how they are printed.

use backtrace::Backtrace;
use std::fmt::Write as _;

/// Set this to anything to print whole stacks instead of workspace frames
/// alone.
pub(super) const WHOLE: &str = "CATCAD_ALLOC_WHOLE_STACK";

/// This crate's own files, which are the harness rather than what it measures.
const HARNESS: &str = "common/src/";

/// The workspace root every member sits under.
///
/// This crate's own directory less its last part: `common` is a member, so its
/// parent is the root. That is what lets a frame in workspace code be told from
/// one in a dependency by its path alone — the paths a stack carries are
/// absolute and say nothing else about who wrote them.
fn root() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
        .rsplit_once('/')
        .map_or("", |(root, _)| root)
}

/// `stack` as the workspace frames of it, outermost first.
///
/// Resolves on the way, the capture having walked the stack and looked nothing
/// up — so a passing test pays for no symbol at all.
pub(super) fn workspace(stack: &mut Backtrace) -> String {
    stack.resolve();
    if std::env::var_os(WHOLE).is_some() {
        return format!("{stack:?}");
    }
    let mut written = String::new();
    for frame in stack.frames() {
        for symbol in frame.symbols() {
            let Some(file) = symbol.filename() else {
                continue;
            };
            let file = file.to_string_lossy();
            let Some(at) = under_root(&file) else {
                continue;
            };
            if at.starts_with(HARNESS) {
                continue;
            }
            let name = symbol
                .name()
                .map_or_else(|| String::from("<unknown>"), |name| format!("{name:#}"));
            let _ = writeln!(
                written,
                "  {name}\n      at {at}:{}:{}",
                symbol.lineno().unwrap_or(0),
                symbol.colno().unwrap_or(0),
            );
        }
    }
    if written.is_empty() {
        written.push_str("(no workspace frames — the whole stack:)\n");
        let _ = write!(written, "{stack:?}");
    }
    written
}

/// `path` relative to the workspace, or `None` where it lies outside it.
fn under_root(path: &str) -> Option<&str> {
    let inside = path.strip_prefix(root())?;
    Some(inside.strip_prefix('/').unwrap_or(inside))
}
