//! Maps from source UTF-8 bytes to rendered text, beside the blocks they map, so find,
//! highlights and selection read source offsets in rendered text.

use super::*;

/// Maps source UTF-8 byte ranges to rendered text UTF-8 byte ranges.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SpanMap(pub(super) Vec<(Range<usize>, Range<usize>)>);

impl SpanMap {
    pub(super) fn push(&mut self, src: Range<usize>, out: Range<usize>) {
        if src.start >= src.end || out.start >= out.end {
            return;
        }
        if let Some((last_src, last_out)) = self.0.last_mut()
            && last_src.end == src.start
            && last_out.end == out.start
            && last_src.len() == last_out.len()
            && src.len() == out.len()
        {
            last_src.end = src.end;
            last_out.end = out.end;
            return;
        }
        self.0.push((src, out));
    }

    pub(super) fn truncate_out(&mut self, len: usize) {
        self.0.retain_mut(|(src, out)| {
            if out.start >= len {
                return false;
            }
            if out.end > len {
                let cut = out.end - len;
                if src.len() == out.len() {
                    src.end = src.end.saturating_sub(cut);
                }
                out.end = len;
            }
            src.start < src.end && out.start < out.end
        });
    }

    pub(super) fn shift_src(&mut self, offset: usize) {
        if offset == 0 {
            return;
        }
        for (src, _) in &mut self.0 {
            src.start += offset;
            src.end += offset;
        }
    }
}

/// Source-to-rendered byte mappings parallel to [`Block`], kept beside `Block` so `Block` keeps
/// its shape.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) enum BlockMap {
    Inline(SpanMap),
    Code(SpanMap),
    List(Vec<Vec<BlockMap>>),
    Quote(Vec<BlockMap>),
    Table { head: Vec<SpanMap>, rows: Vec<Vec<SpanMap>> },
    #[default]
    None,
}

impl BlockMap {
    pub(super) fn shift_src(mut self, offset: usize) -> Self {
        if offset == 0 {
            return self;
        }
        match &mut self {
            BlockMap::Inline(m) | BlockMap::Code(m) => m.shift_src(offset),
            BlockMap::List(items) => {
                for item in items {
                    for b in item {
                        *b = std::mem::take(b).shift_src(offset);
                    }
                }
            }
            BlockMap::Quote(inner) => {
                for b in inner {
                    *b = std::mem::take(b).shift_src(offset);
                }
            }
            BlockMap::Table { head, rows } => {
                for m in head {
                    m.shift_src(offset);
                }
                for row in rows {
                    for m in row {
                        m.shift_src(offset);
                    }
                }
            }
            BlockMap::None => {}
        }
        self
    }
}
