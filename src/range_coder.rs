const N_BITS: usize = 32;
const NORM: u64 = 1 << N_BITS;
pub const MAX_RANGE_DENOMINATOR: u64 = NORM - 1;
const HALF: u64 = NORM / 2;

pub struct RangeEncoder {
    low: u64,
    range: u64,
    out_buf: Vec<bool>,
}

impl RangeEncoder {
    pub fn new() -> Self {
        Self {
            low: 0,
            range: NORM,
            out_buf: Vec::new(),
        }
    }

    fn carry_one(&mut self) {
        for b in self.out_buf.iter_mut().rev() {
            *b = !*b;
            if *b {
                break;
            }
        }
    }

    pub fn encode_range(&mut self, low: u64, high: u64, denominator: u64) {
        while self.range <= HALF {
            self.out_buf.push(self.low >= HALF);

            self.low &= HALF - 1;
            self.low *= 2;
            self.range *= 2;
        }

        let offset = self.range * low / denominator;
        self.low += offset;
        self.range = self.range * high / denominator - offset;

        if self.low >= NORM {
            self.low -= NORM;
            self.carry_one();
        }
    }

    pub fn encode(&mut self, table: &[u64], denominator: u64, symbol: usize) {
        self.encode_range(
            table[symbol],
            *table.get(symbol + 1).unwrap_or(&denominator),
            denominator,
        )
    }

    pub fn flush(mut self) -> Vec<bool> {
        while self.range <= NORM {
            if !(self.low + 1..self.low + self.range).contains(&HALF) || self.low == 0 {
                self.out_buf.push(self.low >= HALF);
                self.low &= HALF - 1;
            } else if self.low + self.range - HALF > HALF - self.low {
                self.range -= HALF - self.low;
                self.low = 0;
                self.out_buf.push(true);
            } else {
                self.range -= self.low + self.range - HALF;
                self.out_buf.push(false);
            }

            self.low *= 2;
            self.range *= 2;
        }

        self.out_buf
    }
}

pub struct RangeDecoder {
    low: u64,
    range: u64,
    in_buf: Vec<bool>,
    buf_pos: usize,
}

impl RangeDecoder {
    pub fn new(in_buf: Vec<bool>) -> Self {
        Self {
            low: 0,
            range: 1,
            in_buf,
            buf_pos: 0,
        }
    }

    fn input_bit(&mut self) -> bool {
        // Pad the output with a single zero bit, then infinite ones.
        let out = self
            .in_buf
            .get(self.buf_pos)
            .copied()
            .unwrap_or(self.buf_pos > self.in_buf.len());
        self.buf_pos += 1;

        out
    }

    fn fill_range(&mut self) {
        while self.range <= HALF {
            self.low = self.low * 2 + self.input_bit() as u64;
            self.range *= 2;
        }
    }

    pub fn selected_symbol(&mut self, table: &[u64], denominator: u64) -> usize {
        self.fill_range();

        table
            .binary_search_by_key(&self.low, |x| x * self.range / denominator)
            .unwrap_or_else(|x| x - 1)
    }

    pub fn decode_range(&mut self, low: u64, high: u64, denominator: u64) {
        self.fill_range();
        eprintln!(
            " {} {} {}",
            -((high - low) as f64 / denominator as f64).log2(),
            self.buf_pos,
            self.in_buf.len()
        );

        let offset = self.range * low / denominator;
        self.low -= offset;
        self.range = self.range * high / denominator - offset;
    }

    pub fn decode(&mut self, table: &[u64], denominator: u64) -> usize {
        let symbol = self.selected_symbol(table, denominator);

        let low = table[symbol];
        let high = table.get(symbol + 1).copied().unwrap_or(denominator);

        self.decode_range(low, high, denominator);

        symbol
    }

    pub fn is_done(&self) -> bool {
        // We are done once the padding 0 is encoded.
        self.buf_pos > self.in_buf.len() + N_BITS + 1
    }
}

fn test_range_coding_case(table: &[u64], denom: u64, message: &[usize]) {
    let mut encoder = RangeEncoder::new();

    for &symbol in message {
        encoder.encode(table, denom, symbol);
    }

    let bits = encoder.flush();

    let mut decoder = RangeDecoder::new(bits.clone());
    let mut message2 = Vec::new();

    while !decoder.is_done() {
        message2.push(decoder.decode(table, denom));
    }

    assert_eq!(message, &message2[..message.len()]);

    encoder = RangeEncoder::new();

    for &symbol in &message2 {
        encoder.encode(table, denom, symbol);
    }

    assert_eq!(bits.as_slice(), &encoder.flush()[..bits.len()])
}

#[test]
fn test_range_coding() {
    test_range_coding_case(&[0, 5, 10, 15], 16, &[0, 3, 2, 3, 3, 3, 2, 1, 3, 0, 1]);
    test_range_coding_case(&[0, 4, 10, 15], 16, &[0, 0, 0, 0, 0]);
    test_range_coding_case(&[0, 5, 10, 15], 16, &[0]);
    test_range_coding_case(&[0, 5, 10, 15], 16, &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
    test_range_coding_case(&[0, 5, 10, 15], 16, &[1]);
    test_range_coding_case(&[0, 5, 10, 15], 16, &[2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2]);
    test_range_coding_case(&[0, 5, 10, 15], 16, &[2]);
    test_range_coding_case(&[0, 5, 10, 15], 16, &[3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3]);
    test_range_coding_case(&[0, 5, 10, 15], 16, &[3]);
    test_range_coding_case(
        &[0, 5, 10, 15],
        NORM - 1,
        &[3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3],
    );
}
