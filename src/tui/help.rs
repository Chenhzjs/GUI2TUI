//! Presentation-only help: no backend operations or semantic ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpContext {
    Scene,
    Reader,
    Outline,
    Search,
    Choice,
    Table,
    SelectableTable,
    Collection,
    Command,
    PublicActions,
    Edit,
    Modality,
    Unavailable,
}
impl HelpContext {
    pub fn text(self) -> &'static str {
        match self {
            Self::Scene => {
                "Start here\n1. Tab / Shift-Tab: move the highlighted control\n2. Enter: use it, open its editor, or read its text\n3. Cannot find an action? F3, type its name, then choose a result\n4. Esc: close a panel or cancel editing; from the main view it quits\n5. F6: move to another region when Tab does not reach it\n\nChanges are sent only when you use an action or apply an edit. Unknown means the result is not confirmed; inspect the current view before trying again.\n\nMore controls\nF6 / Shift-F6: next/previous major region\nCtrl-Tab / Ctrl-Shift-Tab: next/previous pane in the active region group\nTab / Shift-Tab: focus controls in the active pane\nEnter: use the focused control, edit a plain field, choose a value, or read a document\nAlt-Enter: experimental raw Enter to the exact authority-checked focused control (not Semantic Submit)\ne: external edit when a complete writable text target and configured handler are available\nSpace: mapped toggle/action (no anonymous action fallback)\nUp/Down / PageUp/PageDown: scroll the active pane\n: open commands in the current scope\nr: force refresh\nb: choose a current application\nF3: public actions and interface operations\nF4: resources and visual tasks\nq / Esc: quit\n\nRead-only wording means no mapped operation is available; inspect F3 for public operations. Password editing is disabled."
            }
            Self::Reader => {
                "Reader\nj/k or Down/Up: move semantic blocks\nPageDown/PageUp: move ten blocks\no: outline\n/: search loaded content; Ctrl-F there searches progressively\nEnter: open a table or collection at this block\nF4: resources\nEsc: return to Scene"
            }
            Self::Outline => {
                "Outline\nj/k or Down/Up: select heading\nEnter: go to heading\n/: search\nEsc: return to Reader"
            }
            Self::Search => {
                "Reader search\nType: search loaded text\nCtrl-F: start progressive document search\nUp/Down: select match\nEnter: go to match\nEsc: cancel a running search, or return to Reader\nF1: help (question mark remains query text)"
            }
            Self::Choice => {
                "Choice\nj/k or Down/Up: select an option\nHome/End: first/last option\nEnter: request safe selection and confirm from GUI\nEsc: cancel this terminal overlay; GUI unchanged\nFocus returns to the choice owner."
            }
            Self::Table => {
                "Table\nh/j/k/l or arrows: move by semantic row/column\nEsc: return to Reader\nCells not exposed by accessibility remain unavailable."
            }
            Self::SelectableTable => {
                "Table\nh/j/k/l or arrows: move through current accessible cells\nEnter: select the row containing the current cell and confirm it from the GUI\nEsc: return to Reader\nCells not exposed by accessibility remain unavailable."
            }
            Self::Collection => {
                "Collection\nj/k or Down/Up: move through exposed items\nEsc: return to Reader\nPartial collections do not imply all logical items are loaded."
            }
            Self::PublicActions => {
                "Public actions\nType: search public object names, descriptions and operations across all sections\nCurrent object / Controls / Interface operations are presentation groups\nUp/Down: select an operation\nPgUp/PgDn: scroll details\nEnter: use the selected operation\nF2 does not change scope here; the current interaction scope stays fixed\nEsc: close and return\nSource IDs and locators in details are diagnostic references, not shortcuts."
            }
            Self::Command => {
                "Commands\nType: search current scope\nF2: toggle all-scope search\nUp/Down: select\nEnter/Right: execute safe command or open a group\nLeft: parent group when query is empty\nEsc: back or close\nF1: help (question mark remains query text)"
            }
            Self::Edit => {
                "Plain text editing\nType: edit the local buffer (q/r/? are text)\nLeft/Right/Home/End: local cursor\nBackspace/Delete: remove characters\nCtrl-S / Ctrl-Enter: apply the text and check it in the GUI\nEnter: apply the text, then submit only if the application publicly supports submission\nEsc: cancel without writing\nTab: ignored; commit or cancel first\nExternal changes block commit. Password editing is disabled.\nF1: help; this is not remote caret synchronization."
            }
            Self::Modality => {
                "Resources\nUp/Down: select an image, document or media item exposed by the application\nEnter: inspect its reference, or request opening in a connected viewer\nf: enter a file path to send; requires a connected file receiver\nm: save an available file, or explicitly request an image preview\nImage previews are not original files and may include overlapping windows\no: open a saved item in a viewer on this computer, when configured\nx: cancel a pending transfer or capture\nEsc: return without sending or capturing\nNo resource or no connected viewer is a valid state. No transfer or capture happens automatically."
            }
            Self::Unavailable => {
                "Application / accessibility service unavailable\nF5: bounded retry or search for a fresh application generation\nb: return to application selection\nd: diagnostics\nq / Esc: quit\nOld controls remain non-interactive; identities are never reused."
            }
        }
    }
}
pub const GLOBAL: &str = "\n\nF1: help in every context | ?: help outside text input\nF7: workflow center from Scene (optional Jev client; record/review/save/play)\nF12: contents-free runtime status (F12/Esc returns)\nCtrl-C: quit and restore the terminal\nHelp: Up/Down scroll | Esc/F1/? return";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{
        action::UiIntent,
        edit::{EditCommand, key_to_edit_command},
        input::key_to_intent,
    };
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    #[test]
    fn documented_scene_shortcuts_match_dispatch() {
        for (code, intent) in [
            (KeyCode::Tab, UiIntent::FocusNext),
            (KeyCode::F(6), UiIntent::RegionNext),
            (KeyCode::Enter, UiIntent::Activate),
            (KeyCode::Char(':'), UiIntent::OpenCommandPalette),
            (KeyCode::Char('r'), UiIntent::Refresh),
        ] {
            assert_eq!(
                key_to_intent(KeyEvent::new(code, KeyModifiers::NONE)),
                Some(intent)
            );
        }
        assert_eq!(
            key_to_intent(KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL)),
            Some(UiIntent::SubregionNext)
        );
    }
    #[test]
    fn question_mark_in_edit_mode_remains_text() {
        assert!(matches!(
            key_to_edit_command(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)),
            EditCommand::Insert('?')
        ));
        assert!(HelpContext::Edit.text().contains("F1"));
    }
    #[test]
    fn contexts_have_specific_help_not_a_universal_inaccurate_keymap() {
        assert!(HelpContext::Reader.text().contains("Ctrl-F there"));
        assert!(HelpContext::Command.text().contains("F2"));
        assert!(HelpContext::Choice.text().contains("GUI unchanged"));
        assert!(HelpContext::Table.text().contains("row/column"));
        assert!(!HelpContext::Table.text().contains("select the current row"));
        assert!(
            HelpContext::SelectableTable
                .text()
                .contains("select the row containing the current cell")
        );
    }
}
