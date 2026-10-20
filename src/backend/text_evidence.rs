//! Operation-local literal text evidence from public TextChanged signals.
//!
//! Event payloads describe inserted text, while their spans can use UTF-16
//! positions. No representation is inferred from an application identity or
//! from the requested value. A literal is usable only when its complete event
//! projection exactly matches a fresh public Text read.

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TextChange {
    pub insert: bool,
    pub start: usize,
    pub length: usize,
    // A receipt needs the final current value, not deleted/transient content.
    #[serde(skip_serializing)]
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TextReadbackEvidence {
    pub source: &'static str,
    pub literal_text: String,
    pub exposed_text: String,
    pub changes: Vec<TextChange>,
}

pub(crate) fn has_ambiguous_padding(text: &str) -> bool {
    text.chars()
        .zip(text.chars().skip(1))
        .any(|(previous, next)| previous.len_utf16() == 2 && next == '\u{feff}')
}

#[derive(Clone, Debug)]
pub(crate) struct TextChangeProof {
    wire: String,
    // One cell per public offset; None is a padding cell established by an
    // event span. An absent map means the initial representation is ambiguous.
    cells: Option<Vec<Option<char>>>,
    changes: Vec<TextChange>,
    pub revision: u64,
    failed: bool,
}

impl TextChangeProof {
    pub fn new(initial: String) -> Self {
        Self {
            cells: (!has_ambiguous_padding(&initial)).then(|| initial.chars().map(Some).collect()),
            wire: initial,
            changes: Vec::new(),
            revision: 0,
            failed: false,
        }
    }

    pub fn fail(&mut self) {
        self.revision += 1;
        self.failed = true;
        self.cells = None;
    }

    pub fn apply(&mut self, insert: bool, start: i32, length: i32, text: String) {
        self.revision += 1;
        if self.failed {
            return;
        }
        let (Ok(start), Ok(length)) = (usize::try_from(start), usize::try_from(length)) else {
            self.fail();
            return;
        };
        let count = self.wire.chars().count();
        if start > count || (!insert && length > count - start) {
            self.fail();
            return;
        }
        let start_byte = byte_offset(&self.wire, start);
        if insert {
            let Some(fragment) = event_projection(&text, length) else {
                self.fail();
                return;
            };
            if self
                .cells
                .as_ref()
                .is_some_and(|cells| !boundary(cells, start))
            {
                self.cells = None;
            }
            if let Some(cells) = &mut self.cells {
                let mut inserted = Vec::new();
                let padded = text.chars().count() != length;
                for character in text.chars() {
                    inserted.push(Some(character));
                    if padded && character.len_utf16() == 2 {
                        inserted.push(None);
                    }
                }
                cells.splice(start..start, inserted);
            }
            self.wire.insert_str(start_byte, &fragment);
        } else {
            let end_byte = byte_offset(&self.wire, start + length);
            if !text.is_empty()
                && event_projection(&text, length).as_deref()
                    != Some(&self.wire[start_byte..end_byte])
            {
                self.fail();
                return;
            }
            self.wire.replace_range(start_byte..end_byte, "");
            if start == 0 && length == count {
                self.cells = Some(Vec::new());
            } else if self
                .cells
                .as_ref()
                .is_some_and(|cells| !boundary(cells, start) || !boundary(cells, start + length))
            {
                self.cells = None;
            } else if let Some(cells) = &mut self.cells {
                cells.drain(start..start + length);
            }
        }
        self.changes.push(TextChange {
            insert,
            start,
            length,
            text,
        });
    }

    pub fn verify(&self, revision: u64, raw: &str) -> Option<TextReadbackEvidence> {
        if self.failed || revision != self.revision || self.wire != raw {
            return None;
        }
        let literal: String = self.cells.as_ref()?.iter().filter_map(|c| *c).collect();
        if self.changes.is_empty() {
            return None;
        }
        Some(TextReadbackEvidence {
            source: "public_text_changed_and_fresh_text",
            literal_text: literal,
            exposed_text: raw.to_owned(),
            changes: self.changes.clone(),
        })
    }
}

fn boundary(cells: &[Option<char>], offset: usize) -> bool {
    offset == cells.len() || cells[offset].is_some()
}

fn byte_offset(text: &str, offset: usize) -> usize {
    text.char_indices()
        .nth(offset)
        .map_or(text.len(), |(byte, _)| byte)
}

fn event_projection(text: &str, length: usize) -> Option<String> {
    if text.chars().count() == length {
        return Some(text.to_owned());
    }
    if text.encode_utf16().count() != length {
        return None;
    }
    let mut projected = String::new();
    for character in text.chars() {
        projected.push(character);
        if character.len_utf16() == 2 {
            projected.push('\u{feff}');
        }
    }
    Some(projected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_events_and_fresh_wire_must_both_agree() {
        let mut proof = TextChangeProof::new(String::new());
        proof.apply(true, 0, 1, "陈".into());
        proof.apply(true, 1, 5, "小明 🧪".into());
        let evidence = proof.verify(proof.revision, "陈小明 🧪\u{feff}").unwrap();
        assert_eq!(evidence.literal_text, "陈小明 🧪");
        assert!(proof.verify(proof.revision, "陈小明 🧪").is_none());
        assert!(
            proof
                .verify(proof.revision - 1, "陈小明 🧪\u{feff}")
                .is_none()
        );
    }

    #[test]
    fn genuine_feff_is_preserved_and_never_satisfied_by_padding_alone() {
        let mut proof = TextChangeProof::new(String::new());
        proof.apply(true, 0, 3, "🧪\u{feff}".into());
        assert!(proof.verify(proof.revision, "🧪\u{feff}").is_none());
        assert_eq!(
            proof
                .verify(proof.revision, "🧪\u{feff}\u{feff}")
                .unwrap()
                .literal_text,
            "🧪\u{feff}"
        );
        let mut scalar = TextChangeProof::new(String::new());
        scalar.apply(true, 0, 2, "🧪\u{feff}".into());
        assert_eq!(
            scalar
                .verify(scalar.revision, "🧪\u{feff}")
                .unwrap()
                .literal_text,
            "🧪\u{feff}"
        );
    }

    #[test]
    fn old_content_needs_complete_replacement_and_stream_errors_fail_closed() {
        let mut proof = TextChangeProof::new("🧪\u{feff}".into());
        assert!(proof.verify(0, "🧪\u{feff}").is_none());
        proof.apply(false, 0, 2, "🧪".into());
        proof.apply(true, 0, 3, "x\u{feff}y".into());
        assert_eq!(
            proof
                .verify(proof.revision, "x\u{feff}y")
                .unwrap()
                .literal_text,
            "x\u{feff}y"
        );
        proof.fail();
        assert!(proof.verify(proof.revision, "x\u{feff}y").is_none());
    }

    #[test]
    fn receipts_do_not_export_deleted_event_payloads() {
        let old = "old-private-value";
        let mut proof = TextChangeProof::new(old.into());
        proof.apply(false, 0, old.len() as i32, old.into());
        proof.apply(true, 0, 2, "🧪".into());
        let receipt =
            serde_json::to_string(&proof.verify(proof.revision, "🧪\u{feff}").unwrap()).unwrap();
        assert!(!receipt.contains(old));
        assert!(receipt.contains("literal_text"));
    }

    #[test]
    fn minimal_public_diffs_preserve_unchanged_spaces() {
        let mut proof = TextChangeProof::new("Ada Lovelace".into());
        proof.apply(false, 0, 3, "Ada".into());
        proof.apply(true, 0, 3, "陈小明".into());
        proof.apply(false, 4, 8, "Lovelace".into());
        proof.apply(true, 4, 2, "🧪".into());
        assert_eq!(
            proof
                .verify(proof.revision, "陈小明 🧪\u{feff}")
                .unwrap()
                .literal_text,
            "陈小明 🧪"
        );
    }
}
