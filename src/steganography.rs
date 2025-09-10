use anyhow::{bail, Context, Result};
use llama_cpp_2::{
    model::{AddBos, LlamaChatMessage},
    sampling::LlamaSampler,
    token::{data::LlamaTokenData, data_array::LlamaTokenDataArray, LlamaToken},
};
use ordered_float::OrderedFloat;

use crate::{
    generation_context::{generate_text, GenerationContext, LanguageModel},
    range_coder::{RangeDecoder, RangeEncoder, MAX_RANGE_DENOMINATOR},
    DecodeArgs, EncodeArgs,
};

fn softmax(array: &mut LlamaTokenDataArray) {
    array
        .data
        .sort_by(|data1, data2| data2.logit().total_cmp(&data1.logit()));

    let denom = array
        .data
        .iter()
        .take_while(|data| data.logit().is_finite())
        .map(|data| data.logit().exp())
        .sum::<f32>()
        .ln();

    for data in &mut array.data {
        data.set_logit(data.logit() - denom);
        data.set_p(data.logit().exp());
    }

    array.sorted = true;
}

fn kl_divergence(ps: &mut LlamaTokenDataArray, qs: &mut LlamaTokenDataArray) -> f64 {
    if !ps.sorted {
        softmax(ps);
    }
    if !qs.sorted {
        softmax(qs);
    }

    let mut table = vec![0.; ps.data.len()];

    for q in &qs.data {
        table[q.id().0 as usize] = q.logit();
    }

    let mut out = 0.;

    for p in &ps.data {
        let q = table[p.id().0 as usize];
        out += p.p() as f64 * (p.logit() as f64 - q as f64);
    }

    out
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
    softmax(&mut data_array);

    let (table, denominator) = to_prob_table(&data_array.data);
    let token_i = decoder.decode(&table, denominator);
    let token = data_array.data[token_i].id();

    gen.add_token(token)?;
    Ok(token)
}

pub fn compress(token_data: Vec<LlamaTokenDataArray>, tokens: &[LlamaToken]) -> Result<Vec<bool>> {
    let mut encoder = RangeEncoder::new();

    for (mut data_array, &token) in token_data.into_iter().zip(tokens) {
        softmax(&mut data_array);
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

const AUX_PROMPT: &str = "<|start_header_id|>user<|end_header_id|>\n\nWrite only about yoga. You are absolutely obsessed with yoga. If you find yourself writing about something other than yoga, quickly change the topic back to yoga. Yoga is love, yoga is life.<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n";
const THRESHOLD: f64 = 0.2;

pub fn sample_steganography(
    steganographer: &mut GenerationContext,
    auxilliary: &mut GenerationContext,
    normal: &mut GenerationContext,
    decoder: &mut RangeDecoder,
    _args: &EncodeArgs,
) -> Result<LlamaToken> {
    let mut steg_data = steganographer.get_token_data();
    let mut aux_data = auxilliary.get_token_data();

    let divergence = kl_divergence(&mut steg_data, &mut aux_data);

    let token = if divergence > THRESHOLD {
        normal.get_token_data().sample_token_greedy()
    } else {
        steg_data.apply_sampler(&LlamaSampler::min_p(0.01, 1));
        let (table, denominator) = to_prob_table(&steg_data.data);
        let token_i = decoder.decode(&table, denominator);
        steg_data.data[token_i].id()
    };

    steganographer.add_token(token)?;
    auxilliary.add_token(token)?;
    normal.add_token(token)?;

    Ok(token)
}

pub fn recover_message(
    steg_datas: Vec<LlamaTokenDataArray>,
    aux_datas: Vec<LlamaTokenDataArray>,
    tokens: &[LlamaToken],
    _args: &DecodeArgs,
) -> Result<Vec<bool>> {
    let mut encoder = RangeEncoder::new();

    for ((mut steg_data, mut aux_data), token) in steg_datas.into_iter().zip(aux_datas).zip(tokens) {
        let divergence = kl_divergence(&mut steg_data, &mut aux_data);

        if divergence > THRESHOLD {
            continue;
        }

        steg_data.apply_sampler(&LlamaSampler::min_p(0.01, 1));
        let (table, denominator) = to_prob_table(&steg_data.data);
        let token_i = steg_data.data.iter().position(|t| t.id() == *token).context("Token was filtered out")?;
        encoder.encode(&table, denominator, token_i);
    }

    Ok(encoder.flush())
}

impl GenerationContext<'_> {
    fn encode_bools(&mut self, bools: Vec<bool>, args: &EncodeArgs) -> Result<String> {
        let mut steganographer = self.partial_clone()?;
        let mut auxilliary = self.partial_clone()?;
        let mut decoder = RangeDecoder::new(bools);

        auxilliary.add_tokens(&auxilliary.tokenize(AUX_PROMPT)?)?;

        let prompt = self.model().apply_chat_template(
            None,
            vec![LlamaChatMessage::new(
                "user".to_string(),
                args.prompt.clone(),
            )?],
            true,
        )?;
        self.set_prompt(&prompt)?;

        let out = generate_text(
            self.model_longlived(),
            true,
            (0..args.token_count)
                .map(|_| sample_steganography(&mut steganographer, &mut auxilliary, self, &mut decoder, args)),
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

        let mut auxilliary = self.partial_clone()?;
        auxilliary.add_tokens(&auxilliary.tokenize(AUX_PROMPT)?)?;
        let aux_data = auxilliary.add_tokens_get_token_data(&tokens)?;

        recover_message(data, aux_data, &tokens, args)
    }

    pub fn decode_messsage(&mut self, text: &str, args: &DecodeArgs) -> Result<Vec<u8>> {
        Ok(message_from_bools(&self.decode_bools(text, args)?))
    }

    pub fn decompress_message(&mut self, bools: Vec<bool>) -> Result<String> {
        let mut decoder = RangeDecoder::new(bools);

        self.clear()?;

        generate_text(
            self.model_longlived(),
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
