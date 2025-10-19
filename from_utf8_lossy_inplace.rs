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
