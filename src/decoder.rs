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

    /// Adds a token to the decoder and returns a full [`String`] representation
    /// of it if possible.
    ///
    /// If the token has a trailing incomplete UTF-8 sequence, this method will
    /// not include it in the output string. Instead, the incomplete sequence
    /// will be stored in the decoder's buffer for the next call to this method.
    pub fn add_token<'a>(&'a mut self, token: &[u8]) -> &'a str {
        let old_len = self.safe_len;

        for chunk in token.utf8_chunks() {
            if !chunk.valid().is_empty() && self.safe_len < self.buf.len() {
                self.buf.truncate(self.safe_len);
                self.buf.extend_from_slice(REPLACEMENT_CHARACTER_BYTES);
                self.safe_len = self.buf.len();
            }

            self.buf.extend_from_slice(chunk.valid().as_bytes());
            self.safe_len = self.buf.len();
            self.buf.extend_from_slice(chunk.invalid());
        }

        unsafe { std::str::from_utf8_unchecked(&self.buf[old_len..self.safe_len]) }
    }

    /// Returns a [`char::REPLACEMENT_CHARACTER`] if there is an incomplete sequence at the end of
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

    /// Converts the [`TokenDecoder`] into a [`String`], replacing any trailing sequence with a
    /// [`char::REPLACEMENT_CHARACTER`]
    pub fn into_string(mut self) -> String {
        if self.safe_len < self.buf.len() {
            self.buf.truncate(self.safe_len);
            self.buf.extend_from_slice(REPLACEMENT_CHARACTER_BYTES);
        }

        unsafe { String::from_utf8_unchecked(self.buf) }
    }
}
