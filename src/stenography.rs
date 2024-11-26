use anyhow::{bail, Context, Result};
use llama_cpp_2::{
    model::{AddBos, LlamaChatMessage, LlamaModel, Special},
    token::{data::LlamaTokenData, data_array::LlamaTokenDataArray, LlamaToken},
};
use ordered_float::OrderedFloat;

use crate::{
    decoder::TokenDecoder,
    generation_context::GenerationContext,
    range_coder::{RangeDecoder, RangeEncoder, MAX_RANGE_DENOMINATOR},
    DecodeArgs, EncodeArgs,
};

fn coding_windows<'a>(
    array: &'a mut LlamaTokenDataArray,
    args: &DecodeArgs,
) -> impl Iterator<Item = &'a [LlamaTokenData]> {
    array.sample_softmax(None);

    let mut array2 = array.clone();

    array2.sample_min_p(None, args.min_p, 1);
    array2.sample_top_k(None, args.top_k as i32, 1);
    array2.sample_temp(None, args.temp);
    array2.sample_softmax(None);

    array.data[0..array2.data.len()].clone_from_slice(array2.data.as_slice());

    let last_id = array
        .data
        .get(array2.data.len())
        .map(|x| x.id())
        .unwrap_or(LlamaToken(-1));
    let mut first_chunk = true;

    array.data.chunk_by(move |_, d| {
        if d.id() == last_id {
            first_chunk = false;
        }
        first_chunk
    })
}

fn to_prob_table(data: &[LlamaTokenData]) -> (Vec<u64>, u64) {
    let total_prob = data.iter().map(|d| d.p() as f64).sum::<f64>();

    let mut out = Vec::new();
    let mut sum = 0;

    for d in data {
        out.push(sum);
        sum += ((d.p() as f64 / total_prob * MAX_RANGE_DENOMINATOR as f64) as u64).max(1);
    }

    (out, sum.max(1))
}

pub fn recover_message(
    token_data: Vec<LlamaTokenDataArray>,
    tokens: &[LlamaToken],
    args: &DecodeArgs,
) -> Vec<bool> {
    let mut encoder = RangeEncoder::new();

    for (mut data_array, &token) in token_data.into_iter().zip(tokens).skip(args.skip_start) {
        let window = coding_windows(&mut data_array, args)
            .find(|window| window.iter().any(|d| d.id() == token))
            .expect("No window contains the token");
        let token_i = window
            .iter()
            .position(|d| d.id() == token)
            .expect("The window does not contain the token!");

        let (table, denominator) = to_prob_table(window);
        encoder.encode(&table, denominator, token_i);
    }

    encoder.flush()
}

pub fn sample_stenography(
    normal: &mut GenerationContext,
    stenographer: &mut GenerationContext,
    decoder: &mut RangeDecoder,
    args: &EncodeArgs,
) -> Result<LlamaToken> {
    if stenographer.tokens().len() <= args.skip_start {
        let mut tokens = normal.get_token_data();
        tokens.sample_softmax(None);

        let token = tokens.data[0].id();
        normal.add_token(token)?;
        stenographer.add_token(token)?;

        return Ok(token);
    }

    if decoder.is_done() {
        let mut tokens = normal.get_token_data();
        tokens.sample_softmax(None);

        let token = tokens.data[0].id();
        normal.add_token(token)?;

        return Ok(token);
    }

    let logits = normal.get_logits();
    let mut data_array = stenographer.get_token_data();

    let window = coding_windows(&mut data_array, &args.as_decode_args())
        .max_by_key(|window| {
            let (table, denominator) = to_prob_table(window);
            let token_i = decoder.selected_symbol(&table, denominator);
            let token = window[token_i].id();

            OrderedFloat(logits[token.0 as usize])
        })
        .context("No windows")?;

    let (table, denominator) = to_prob_table(window);
    let token_i = decoder.decode(&table, denominator);
    let token = window[token_i].id();

    normal.add_token(token)?;
    stenographer.add_token(token)?;

    Ok(token)
}

pub fn sample_decompress(
    gen: &mut GenerationContext,
    decoder: &mut RangeDecoder,
) -> Result<LlamaToken> {
    if decoder.is_done() {
        // For a correctly-compressed message, this should never run, but if the message is
        // corrupted, we should stop quickly.
        return Ok(gen.model().token_eos());
    }

    let mut data_array = gen.get_token_data();
    data_array.sample_softmax(None);

    let (table, denominator) = to_prob_table(&data_array.data);
    let token_i = decoder.decode(&table, denominator);
    let token = data_array.data[token_i].id();

    gen.add_token(token)?;
    Ok(token)
}

pub fn compress(token_data: Vec<LlamaTokenDataArray>, tokens: &[LlamaToken]) -> Result<Vec<bool>> {
    let mut encoder = RangeEncoder::new();

    for (mut data_array, &token) in token_data.into_iter().zip(tokens) {
        data_array.sample_softmax(None);
        let token_i = data_array
            .data
            .iter()
            .position(|d| d.id() == token)
            .expect("The data array does not contain the token!");

        let (table, denominator) = to_prob_table(&data_array.data);
        encoder.encode(&table, denominator, token_i);
    }

    Ok(encoder.flush())
}

pub fn message_to_bools(mut message: Vec<u8>) -> Vec<bool> {
    let length = message.len() as u32;
    message.splice(0..0, length.to_le_bytes());

    message
        .iter()
        .flat_map(|b| (0..8).map(move |i| b & (1 << i) != 0))
        .collect()
}

pub fn message_from_bools(bools: &[bool]) -> Vec<u8> {
    let mut out: Vec<u8> = bools
        .chunks_exact(8)
        .map(|c| {
            c.iter()
                .enumerate()
                .fold(0, |a, (i, b)| a | ((*b as u8) << i))
        })
        .collect();

    let length = u32::from_le_bytes(out[0..4].try_into().unwrap());
    out.drain(0..4);
    out.truncate(length as usize);

    out
}

fn generate_tokens(
    model: &LlamaModel,
    tokens: impl Iterator<Item = Result<LlamaToken>>,
) -> Result<Vec<LlamaToken>> {
    tokens
        .take_while(|t| t.as_ref().map_or(true, |t| !model.is_eog_token(*t)))
        .collect()
}

fn generate_text(
    model: &LlamaModel,
    preview: bool,
    tokens: impl Iterator<Item = Result<LlamaToken>>,
) -> Result<String> {
    let mut token_decoder = TokenDecoder::new();
    let mut out = String::new();

    let tokens = tokens.map(|t| {
        let Ok(t) = t else {
            return t;
        };

        let piece = token_decoder.add_token(&model.token_to_bytes(t, Special::Tokenize)?);

        if preview {
            print!("{piece}");
            std::io::Write::flush(&mut std::io::stdout())?;
        }

        out += &piece;

        Ok(t)
    });

    generate_tokens(model, tokens)?;

    if let Some(piece) = token_decoder.last_part() {
        print!("{piece}");
        out += &piece;
    }

    println!();

    Ok(out)
}

fn apply_chat_template_hack(
    model: &LlamaModel,
    mut messages: Vec<LlamaChatMessage>,
) -> Result<String> {
    let user_message = LlamaChatMessage::new(
        "user".to_string(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
    )?;
    let assistant_message = LlamaChatMessage::new(
        "assistant".to_string(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
    )?;

    messages.insert(0, user_message.clone());
    messages.insert(1, assistant_message.clone());
    let res1 = model.apply_chat_template(None, messages.clone(), true)?;

    messages.insert(0, user_message);
    messages.insert(1, assistant_message);
    let res2 = model.apply_chat_template(None, messages, true)?;

    let len = res2.len() - res1.len();
    Ok(res1[len..].to_string())
}

impl GenerationContext<'_> {
    fn encode_bools(&mut self, bools: Vec<bool>, args: &EncodeArgs) -> Result<String> {
        let mut stenographer = self.partial_clone()?;
        let mut decoder = RangeDecoder::new(bools);

        let prompt = apply_chat_template_hack(
            self.model(),
            vec![LlamaChatMessage::new(
                "user".to_string(),
                args.prompt.clone(),
            )?],
        )?;
        self.set_prompt(&prompt)?;

        let out = generate_text(
            self.model(),
            true,
            (0..args.token_count)
                .map(|_| sample_stenography(self, &mut stenographer, &mut decoder, args)),
        )?;

        if !decoder.is_done() {
            bail!("Could not encode entire message!");
        }

        Ok(out)
    }

    pub fn encode_message(&mut self, message: Vec<u8>, args: &EncodeArgs) -> Result<String> {
        self.encode_bools(message_to_bools(message), args)
    }

    pub fn encode_compressed(&mut self, message: &str, args: &EncodeArgs) -> Result<String> {
        let bools = self.compress_message(message)?;
        eprintln!("COMPRESSION: {} {}", message.len() * 8, bools.len());
        self.encode_bools(bools, args)
    }

    pub fn decode_bools(&mut self, text: &str, args: &DecodeArgs) -> Result<Vec<bool>> {
        self.clear()?;
        let tokens = self.model().str_to_token(text, AddBos::Never)?;
        let data = self.add_tokens_get_token_data(&tokens)?;

        Ok(recover_message(data, &tokens, args))
    }

    pub fn decode_messsage(&mut self, text: &str, args: &DecodeArgs) -> Result<Vec<u8>> {
        Ok(message_from_bools(&self.decode_bools(text, args)?))
    }

    pub fn decompress_message(&mut self, bools: Vec<bool>) -> Result<String> {
        let mut decoder = RangeDecoder::new(bools);

        self.clear()?;

        generate_text(
            self.model(),
            true,
            (0..1024).map(|_| sample_decompress(self, &mut decoder)),
        )
    }

    pub fn decode_compressed(&mut self, text: &str, args: &DecodeArgs) -> Result<String> {
        let bools = self.decode_bools(text, args)?;
        self.decompress_message(bools)
    }

    pub fn compress_message(&mut self, message: &str) -> Result<Vec<bool>> {
        self.clear()?;

        let mut tokens = self.model().str_to_token(message, AddBos::Always)?;
        tokens.push(self.model().token_eos());

        let data = self.add_tokens_get_token_data(&tokens)?;

        compress(data, &tokens)
    }
}
