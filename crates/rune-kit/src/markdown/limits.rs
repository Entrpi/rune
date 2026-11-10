//! How much of a document Markdown builds, so a broken or hostile source (a model's reply with
//! 100,000 nested `>`) can't overflow the recursive render or grow without bound. Past the depth
//! and block caps the parse degrades and keeps the text. Whole documents and streams parse
//! through one builder, and a stream grown from its last block parses whole once it nears the
//! block cap, so the two always agree.

use super::*;

/// Quotes and lists open inside one another. A deeper one, and every container in it, adds its
/// blocks to the innermost open one, so the render, the block-map walks and the tree's drop,
/// all recursive, stay this many levels deep.
pub(super) const MAX_DEPTH: usize = 32;
/// Blocks at any depth, list items and table rows in one document; the kit's largest test
/// document (1 MB of sections) has 28,832. From the block, item or row that would pass it, the
/// rest of the source is one paragraph of its text as written, and an image in a paragraph
/// stays in its line as its alt text.
pub(super) const MAX_BLOCKS: usize = 100_000;
/// Columns in a table; the kit's widest test table has 10. A row's cells past the last are
/// dropped, as GFM drops a row's cells past its header's.
pub(super) const MAX_COLUMNS: usize = 128;

/// What `blocks` count against [`MAX_BLOCKS`]: every block, list item and table row.
pub(super) fn spent(blocks: &[Block]) -> usize {
    blocks.iter().map(|b| 1 + match b {
        Block::List { items, .. } => items.iter().map(|item| 1 + spent(item)).sum(),
        Block::Quote(inner) => spent(inner),
        Block::Table(t) => t.rows.len(),
        _ => 0,
    }).sum()
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod tests;
