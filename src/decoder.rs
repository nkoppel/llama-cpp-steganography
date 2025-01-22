const REPLACEMENT_CHARACTER_STR: &str = "\u{FFFD}";
const REPLACEMENT_CHARACTER_BYTES: &[u8] = REPLACEMENT_CHARACTER_STR.as_bytes();

/// A struct used to decode [`Vec<u8>`] tokens into [`String`] tokens.
///
/// This struct can handle merging split UTF-8 codepoints.
pub struct TokenDecoder {
    buf: Vec<u8>,
    safe_len: usize,
}

impl TokenDecoder {
    /// Creates a new [`TokenDecoder`].
    pub fn new() -> TokenDecoder {
        TokenDecoder {
            buf: Vec::new(),
            safe_len: 0,
        }
    }

    /// Adds a byte to the decoder, and outputs a UTF-8 character if one was completed.
    pub fn add_byte(&mut self, byte: u8) -> &'_ str {
        let old_len = self.safe_len;

        self.buf.push(byte);
        match std::str::from_utf8(&self.buf[self.safe_len..]) {
            Ok(_) => self.safe_len = self.buf.len(),
            Err(e) if e.error_len().is_some() => {
                self.buf.truncate(self.safe_len);
                self.buf.extend_from_slice(REPLACEMENT_CHARACTER_BYTES);
                self.safe_len = self.buf.len();
            }
            Err(_) => {}
        }

        unsafe { std::str::from_utf8_unchecked(&self.buf[old_len..self.safe_len]) }
    }

    /// Adds a token to the decoder. Returns a [`str`] containing all UTF-8 characters completed by
    /// this token.
    pub fn add_token<'a>(&'a mut self, token: &[u8]) -> &'a str {
        let old_len = self.safe_len;

        for b in token {
            self.add_byte(*b);
        }

        unsafe { std::str::from_utf8_unchecked(&self.buf[old_len..self.safe_len]) }
    }

    /// Returns a [`char::REPLACEMENT_CHARACTER`] if there is a trailing incomplete character in
    /// the decoder's buffer
    pub fn last_part(&self) -> &'static str {
        if self.safe_len < self.buf.len() {
            REPLACEMENT_CHARACTER_STR
        } else {
            ""
        }
    }

    /// Returns the decoded part of the buffer as a string slice
    pub fn buffer(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.buf[..self.safe_len]) }
    }

    /// Converts the [`TokenDecoder`] into a [`String`], replacing any trailing incomplete
    /// character with a [`char::REPLACEMENT_CHARACTER`]
    pub fn into_string(mut self) -> String {
        if self.safe_len < self.buf.len() {
            self.buf.truncate(self.safe_len);
            self.buf.extend_from_slice(REPLACEMENT_CHARACTER_BYTES);
        }

        unsafe { String::from_utf8_unchecked(self.buf) }
    }
}
