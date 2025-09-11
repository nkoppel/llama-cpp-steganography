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
