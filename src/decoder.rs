use crate::improved_utf8_chunks::*;

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
        match std::str::from_utf8(&self.buf[self.safe_len..]).map_err(|e| e.error_len()) {
            Ok(_) => self.safe_len = self.buf.len(),
            Err(Some(len)) => {
                self.buf.splice(
                    self.safe_len..self.safe_len + len,
                    REPLACEMENT_CHARACTER_BYTES.iter().copied(),
                );

                self.safe_len += REPLACEMENT_CHARACTER_BYTES.len();

                if std::str::from_utf8(&self.buf[self.safe_len..]).is_ok() {
                    self.safe_len = self.buf.len();
                }
            }
            Err(None) => {}
        }

        unsafe { std::str::from_utf8_unchecked(&self.buf[old_len..self.safe_len]) }
    }

    /// Adds a token to the decoder. Returns a [`str`] containing all UTF-8 characters completed by
    /// this token.
    pub fn add_token<'a>(&'a mut self, mut token: &[u8]) -> &'a str {
        let old_len = self.safe_len;

        while !token.is_empty() && self.safe_len < self.buf.len() {
            self.add_byte(token[0]);
            token = &token[1..];
        }

        for chunk in utf8_chunks(token) {
            self.buf.extend_from_slice(chunk.valid().as_bytes());
            if chunk.unexpected_end() {
                self.safe_len = self.buf.len();
                self.buf.extend_from_slice(chunk.invalid());
            } else {
                self.buf.extend_from_slice(REPLACEMENT_CHARACTER_BYTES);
                self.safe_len = self.buf.len();
            }
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

pub fn from_utf8_lossy_inplace(mut buf: Vec<u8>) -> String {
    let mut i = 0;
    let mut j = 0;

    while i < buf.len() {
        match std::str::from_utf8(&buf[i..]) {
            Err(error) => {
                i += error.valid_up_to();
                j += error.valid_up_to();

                let error_len = error.error_len().unwrap_or(buf.len() - i);
                buf[i..i + error_len - 1].fill(0xfe);
                buf[i + error_len - 1] = 0xff;

                i += error_len;
                j += 3;
            }
            Ok(s) => {
                i += s.len();
                j += s.len();
            }
        }
    }

    buf.resize(j, 0);

    while i != j {
        i -= 1;
        j -= 1;

        if buf[i] == 0xff {
            while i > 0 && buf[i - 1] == 0xfe {
                i -= 1;
            }

            j -= 2;
            buf[j..j + 3].clone_from_slice(REPLACEMENT_CHARACTER_BYTES);

            continue;
        }

        buf[j] = buf[i];
    }

    unsafe { String::from_utf8_unchecked(buf) }
}

#[test]
fn test_from_utf8_lossy_inplace() {
    let xs = b"hello".to_vec();
    assert_eq!(from_utf8_lossy_inplace(xs), String::from("hello"));

    let xs = "ศไทย中华Việt Nam".as_bytes().to_vec();
    assert_eq!(
        from_utf8_lossy_inplace(xs),
        String::from("ศไทย中华Việt Nam")
    );

    let xs = b"Hello\xC2 There\xFF Goodbye".to_vec();
    assert_eq!(
        from_utf8_lossy_inplace(xs),
        String::from("Hello\u{FFFD} There\u{FFFD} Goodbye")
    );

    let xs = b"Hello\xC0\x80 There\xE6\x83 Goodbye".to_vec();
    assert_eq!(
        from_utf8_lossy_inplace(xs),
        String::from("Hello\u{FFFD}\u{FFFD} There\u{FFFD} Goodbye")
    );

    let xs = b"\xF5foo\xF5\x80bar".to_vec();
    assert_eq!(
        from_utf8_lossy_inplace(xs),
        String::from("\u{FFFD}foo\u{FFFD}\u{FFFD}bar")
    );

    let xs = b"\xF1foo\xF1\x80bar\xF1\x80\x80baz".to_vec();
    assert_eq!(
        from_utf8_lossy_inplace(xs),
        String::from("\u{FFFD}foo\u{FFFD}bar\u{FFFD}baz")
    );

    let xs = b"\xF4foo\xF4\x80bar\xF4\xBFbaz".to_vec();
    assert_eq!(
        from_utf8_lossy_inplace(xs),
        String::from("\u{FFFD}foo\u{FFFD}bar\u{FFFD}\u{FFFD}baz")
    );

    let xs = b"\xF0\x80\x80\x80foo\xF0\x90\x80\x80bar".to_vec();
    assert_eq!(
        from_utf8_lossy_inplace(xs),
        String::from("\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}foo\u{10000}bar")
    );

    // surrogates
    let xs = b"\xED\xA0\x80foo\xED\xBF\xBFbar".to_vec();
    assert_eq!(
        from_utf8_lossy_inplace(xs),
        String::from("\u{FFFD}\u{FFFD}\u{FFFD}foo\u{FFFD}\u{FFFD}\u{FFFD}bar")
    );
}
