//! Single-line widget editing and clipboard input.

use std::ops::Range;

use crate::window::event::{Event, Key};

#[derive(Clone, Copy)]
pub(crate) struct TextEdit {
    pub cursor: usize,
    pub anchor: usize,
}

impl TextEdit {
    pub(super) fn selection(self) -> Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }

    pub(super) fn move_to(&mut self, cursor: usize, extend: bool) {
        self.cursor = cursor;
        if !extend {
            self.anchor = cursor;
        }
    }
}

#[derive(Default)]
pub(super) struct TextInput {
    events: Vec<Event>,
    // Keep ownership alive so other Linux applications can request copied text.
    clipboard: Option<arboard::Clipboard>,
}

impl TextInput {
    pub(super) fn update(&mut self, events: &[Event]) {
        self.events.clear();
        self.events.extend(
            events
                .iter()
                .filter(|event| matches!(event, Event::KeyPress { .. } | Event::Text { .. }))
                .cloned(),
        );
    }

    fn clipboard(&mut self) -> Result<&mut arboard::Clipboard, arboard::Error> {
        if self.clipboard.is_none() {
            self.clipboard = Some(arboard::Clipboard::new()?);
        }
        match self.clipboard.as_mut() {
            Some(clipboard) => Ok(clipboard),
            None => unreachable!("clipboard initialized above"),
        }
    }

    pub(super) fn edit(&mut self, text: &mut String, state: &mut TextEdit) -> (bool, bool) {
        let mut chars: Vec<char> = text.chars().collect();
        state.cursor = state.cursor.min(chars.len());
        state.anchor = state.anchor.min(chars.len());
        let mut submitted = false;
        // Events must keep their order: select-all followed by typing replaces the
        // selection even when both arrive before a single rendered frame.
        for index in 0..self.events.len() {
            match self.events[index].clone() {
                Event::Text { text } => {
                    replace(&mut chars, state, &text);
                }
                Event::KeyPress { key, modifiers, .. } => {
                    let selected = state.selection();
                    if modifiers.ctrl && !modifiers.alt {
                        match key {
                            Key::A => {
                                state.anchor = 0;
                                state.cursor = chars.len();
                            }
                            Key::C | Key::X if !selected.is_empty() => {
                                let copied: String = chars[selected].iter().collect();
                                match self
                                    .clipboard()
                                    .and_then(|clipboard| clipboard.set_text(copied))
                                {
                                    Ok(()) => {
                                        if key == Key::X {
                                            replace(&mut chars, state, "");
                                        }
                                    }
                                    Err(error) => tracing::warn!(%error, "Could not copy text"),
                                }
                            }
                            Key::V => match self.clipboard().and_then(arboard::Clipboard::get_text)
                            {
                                Ok(pasted) if !pasted.is_empty() => {
                                    replace(&mut chars, state, &pasted);
                                }
                                Ok(_) | Err(arboard::Error::ContentNotAvailable) => {}
                                Err(error) => tracing::warn!(%error, "Could not paste text"),
                            },
                            _ => {}
                        }
                        continue;
                    }
                    match key {
                        Key::Backspace | Key::Delete if !selected.is_empty() => {
                            replace(&mut chars, state, "");
                        }
                        Key::Backspace if state.cursor > 0 => {
                            state.anchor = state.cursor - 1;
                            replace(&mut chars, state, "");
                        }
                        Key::Delete if state.cursor < chars.len() => {
                            state.anchor = state.cursor + 1;
                            replace(&mut chars, state, "");
                        }
                        Key::Left => {
                            let cursor = if !modifiers.shift && !selected.is_empty() {
                                selected.start
                            } else {
                                state.cursor.saturating_sub(1)
                            };
                            state.move_to(cursor, modifiers.shift);
                        }
                        Key::Right => {
                            let cursor = if !modifiers.shift && !selected.is_empty() {
                                selected.end
                            } else {
                                (state.cursor + 1).min(chars.len())
                            };
                            state.move_to(cursor, modifiers.shift);
                        }
                        Key::Home => state.move_to(0, modifiers.shift),
                        Key::End => state.move_to(chars.len(), modifiers.shift),
                        Key::Enter => submitted = true,
                        Key::Escape => break,
                        _ => {}
                    }
                }
                _ => unreachable!("only editing events are retained"),
            }
        }
        let edited: String = chars.iter().collect();
        let changed = *text != edited;
        if changed {
            *text = edited;
        }
        (changed, submitted)
    }
}

fn replace(chars: &mut Vec<char>, state: &mut TextEdit, text: &str) {
    let inserted: Vec<char> = text.chars().filter(|c| !c.is_control()).collect();
    // Empty clipboard or filtered control characters must not erase a selection.
    if !text.is_empty() && inserted.is_empty() {
        return;
    }
    let selected = state.selection();
    let cursor = selected.start + inserted.len();
    drop(chars.splice(selected, inserted));
    state.move_to(cursor, false);
}
