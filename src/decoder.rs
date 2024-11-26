/// A struct used to decode [`Vec<u8>`] tokens from a into [`String`] tokens.
///
/// This struct can handle merging split UTF-8 codepoints.
pub struct TokenDecoder {
    /// A buffer used to store incomplete codepoints between calls to
    /// [`TokenDecoder::add_token`]
    buf: Vec<u8>,
}

impl TokenDecoder {
    /// Creates a new [`TokenDecoder`].
    pub fn new() -> TokenDecoder {
        TokenDecoder {
            buf: Vec::with_capacity(64),
        }
    }

    /// Adds a token to the decoder and returns a full [`String`] representation
    /// of it if possible.
    ///
    /// If the token has a trailing incomplete UTF-8 sequence, this method will
    /// not include it in the output string. Instead, the incomplete sequence
    /// will be stored in the decoder's buffer for the next call to this method.
    pub fn add_token(&mut self, token: &[u8]) -> String {
        let mut token = token;
        let mut out = String::with_capacity(self.buf.len() + token.len());

        if !self.buf.is_empty() {
            self.buf.extend_from_slice(token);
            token = self.buf.as_slice();
        }

        loop {
            match std::str::from_utf8(token) {
                Ok(s) => {
                    out.push_str(s);
                    self.buf.clear();
                    return out;
                }
                Err(err) => {
                    let valid_len = err.valid_up_to();
                    out.push_str(unsafe { std::str::from_utf8_unchecked(&token[..valid_len]) });

                    if let Some(len) = err.error_len() {
                        out.push(char::REPLACEMENT_CHARACTER);
                        token = &token[valid_len + len..];
                        continue;
                    }

                    let mut last_bytes = [0; 4];
                    let last_part_len = token.len() - valid_len;
                    last_bytes[..last_part_len].clone_from_slice(&token[valid_len..]);

                    self.buf.clear();
                    self.buf.extend_from_slice(&last_bytes[..last_part_len]);

                    return out;
                }
            }
        }
    }

    /// Returns the last partial UTF-8 sequence stored in the decoder.
    ///
    /// If there is no partial UTF-8 sequence stored, this method will return `None`.
    pub fn last_part(&mut self) -> Option<String> {
        (!self.buf.is_empty()).then(|| {
            let out = String::from_utf8_lossy(&self.buf).to_string();
            self.buf.clear();
            out
        })
    }
}
