const REPLACEMENT_CHARACTER_BYTES: &[u8] = "\u{FFFD}".as_bytes();

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
    pub fn add_token<'a>(&'a mut self, mut token: &[u8]) -> &'a str {
        let old_len = self.safe_len;

        while !token.is_empty() {
            let Err(err) = std::str::from_utf8(&self.buf[self.safe_len..]) else {
                break;
            };
            if err.error_len().is_some() {
                self.buf.truncate(self.safe_len);
                self.buf.extend_from_slice(REPLACEMENT_CHARACTER_BYTES);
                break;
            }

            self.buf.push(token[0]);
            token = &token[1..];
        }

        while let Err(err) = std::str::from_utf8(token) {
            let valid_len = err.valid_up_to();
            self.buf.extend_from_slice(&token[..valid_len]);

            if let Some(len) = err.error_len() {
                self.buf.extend_from_slice(REPLACEMENT_CHARACTER_BYTES);
                token = &token[valid_len + len..];
            } else {
                self.safe_len = self.buf.len();
                self.buf.extend_from_slice(&token[valid_len..]);
                return unsafe { std::str::from_utf8_unchecked(&self.buf[old_len..self.safe_len]) };
            }
        }

        self.buf.extend_from_slice(token);
        self.safe_len = self.buf.len();
        unsafe { std::str::from_utf8_unchecked(&self.buf[old_len..]) }
    }

    /// Returns the last partial UTF-8 sequence stored in the decoder.
    ///
    /// If there is no partial UTF-8 sequence stored, this method will return `None`.
    pub fn last_part(&self) -> String {
        String::from_utf8_lossy(&self.buf[self.safe_len..]).to_string()
    }

    pub fn buffer(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.buf[..self.safe_len]) }
    }

    pub fn into_string(mut self) -> String {
        if self.safe_len < self.buf.len() {
            self.buf.truncate(self.safe_len);
            self.buf.extend_from_slice(REPLACEMENT_CHARACTER_BYTES);
        }

        unsafe { String::from_utf8_unchecked(self.buf) }
    }
}
