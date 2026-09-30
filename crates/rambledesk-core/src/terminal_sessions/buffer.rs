use super::{ApplicationError, MAX_TERMINAL_OUTPUT_BYTES, unavailable};

#[derive(Default)]
pub(super) struct OutputBuffer {
    pub text: String,
    pub next: u64,
    pub truncated: bool,
    pending: Vec<u8>,
}

impl OutputBuffer {
    pub fn append(&mut self, bytes: &[u8], eof: bool) {
        self.pending.extend_from_slice(bytes);
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(text) => {
                    let decoded = text.to_owned();
                    self.pending.clear();
                    self.push(&decoded);
                    break;
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    if valid > 0 {
                        let text = String::from_utf8(self.pending.drain(..valid).collect())
                            .expect("valid UTF-8 prefix");
                        self.push(&text);
                    }
                    match error.error_len() {
                        Some(length) => {
                            self.pending.drain(..length);
                            self.push("\u{fffd}");
                        }
                        None if eof => {
                            self.pending.clear();
                            self.push("\u{fffd}");
                            break;
                        }
                        None => break,
                    }
                }
            }
        }
    }

    fn push(&mut self, text: &str) {
        let without_nul = text.replace('\0', "");
        let text = without_nul.as_str();
        self.next += text.len() as u64;
        self.text.push_str(text);
        if self.text.len() > MAX_TERMINAL_OUTPUT_BYTES {
            let mut remove = self.text.len() - MAX_TERMINAL_OUTPUT_BYTES;
            while !self.text.is_char_boundary(remove) {
                remove += 1;
            }
            self.text.drain(..remove);
            self.truncated = true;
        }
    }

    pub fn read(&self, cursor: Option<u64>) -> Result<(String, u64), ApplicationError> {
        let first = self.next - self.text.len() as u64;
        let start = cursor.unwrap_or(first).max(first);
        if start > self.next {
            return Err(unavailable(
                "Terminal output cursor is ahead of this session.",
            ));
        }
        let offset = (start - first) as usize;
        if !self.text.is_char_boundary(offset) {
            return Err(unavailable(
                "Terminal output cursor splits a UTF-8 character.",
            ));
        }
        Ok((self.text[offset..].to_owned(), start))
    }
}
