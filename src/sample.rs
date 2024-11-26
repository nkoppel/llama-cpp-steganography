use anyhow::Result;
use llama_cpp_2::{
    context::LlamaContext,
    token::{data_array::LlamaTokenDataArray, LlamaToken},
};

#[derive(Clone, Debug)]
pub enum SamplerStage {
    Temperature(f32),
    RepetitionPenalty {
        repetition_penalty: f32,
        frequency_penalty: f32,
        presence_penalty: f32,
    },
    TopP(f32),
    MinP(f32),
    TopK(i32),
    Typical(f32),
    TailFree(f32),
    Dry {
        allowed_length: usize,
        multiplier: f32,
        base: f32,
        sequence_breakers: Vec<String>,
    },
    Xtc {
        probability: f32,
        threshold: f32,
    },
}

#[derive(Clone, Debug)]
pub struct Sampler {
    pub stages: Vec<SamplerStage>,
    pub rep_pen_range: usize,
    pub min_keep: usize,
}

fn sample_dry(
    context: &LlamaContext,
    data_array: &mut LlamaTokenDataArray,
    tokens: &[LlamaToken],
    rep_pen_range: usize,
    stage: &SamplerStage,
) {
    let SamplerStage::Dry {
        allowed_length,
        multiplier,
        base,
        sequence_breakers,
    } = stage
    else {
        panic!("sample_dry was called with invalid arguments!");
    };
    let tokens = &tokens[tokens.len().saturating_sub(rep_pen_range)..];

    let strings = tokens
        .iter()
        .map(|tok| {
            context
                .model
                .token_to_bytes(*tok, llama_cpp_2::model::Special::Tokenize)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let string = strings.iter().flatten().copied().collect::<Vec<_>>();
    let breaker_loc = sequence_breakers
        .iter()
        .map(|br| {
            if let Some(loc) = memchr::memmem::rfind(&string, br.as_bytes()) {
                loc + br.len()
            } else {
                0
            }
        })
        .max()
        .unwrap_or(0);

    let mut i = 0;
    let breaker_token = strings
        .iter()
        .position(|s| {
            i += s.len();
            i >= breaker_loc
        })
        .unwrap_or(tokens.len());

    use std::collections::HashMap;
    let mut active_matches: HashMap<LlamaToken, usize> = HashMap::new();
    let mut matches: HashMap<LlamaToken, usize> = HashMap::new();

    let Some(last_token) = tokens.last() else {
        return;
    };
    if tokens.len() < 2 {
        return;
    }
    for (i, token) in tokens.iter().enumerate().rev().skip(1) {
        active_matches.retain(|token2, j| {
            if tokens[*j] == *token && *j > breaker_token + 1 {
                *j -= 1;
                true
            } else {
                let match_size = matches.entry(*token2).or_default();
                *match_size = (*match_size).max(tokens.len() - *j);
                false
            }
        });

        if token == last_token {
            active_matches.insert(tokens[i + 1], tokens.len() - 2);
        }
    }
    for (token2, j) in active_matches.iter() {
        let match_size = matches.entry(*token2).or_default();
        *match_size = (*match_size).max(tokens.len() - *j);
    }

    for data in data_array.data.iter_mut() {
        let Some(length) = matches.get(&data.id()) else {
            continue;
        };

        if length >= allowed_length {
            let penalty = multiplier * base.powi((*length - allowed_length) as i32);
            data.set_logit(data.logit() - penalty);
        }
    }
}

fn sample_xtc(data_array: &mut LlamaTokenDataArray, probability: f32, threshold: f32) {
    if rand::random::<f32>() < probability {
        return;
    }

    data_array.sample_softmax(None);

    let Some(zero_pos) = data_array
        .data
        .iter()
        .rposition(|data| data.p() > threshold)
    else {
        return;
    };

    for data in &mut data_array.data[0..zero_pos] {
        data.set_logit(f32::NEG_INFINITY);
    }

    data_array.sorted = false;
}

impl SamplerStage {
    pub fn apply(
        &self,
        rep_pen_range: usize,
        min_keep: usize,
        context: &mut LlamaContext,
        mut data_array: LlamaTokenDataArray,
        tokens: &[LlamaToken],
    ) -> Result<LlamaTokenDataArray> {
        match self {
            SamplerStage::Temperature(t) => {
                context.sample_temp(&mut data_array, *t);
            }
            SamplerStage::RepetitionPenalty {
                repetition_penalty,
                frequency_penalty,
                presence_penalty,
            } => {
                data_array.sample_repetition_penalty(
                    None,
                    tokens,
                    rep_pen_range,
                    *repetition_penalty,
                    *frequency_penalty,
                    *presence_penalty,
                );
            }
            SamplerStage::TopP(p) => {
                data_array.sample_top_p(None, *p, min_keep);
            }
            SamplerStage::MinP(p) => {
                data_array.sample_min_p(None, *p, min_keep);
            }
            SamplerStage::TopK(k) => {
                data_array.sample_top_k(None, *k, min_keep);
            }
            SamplerStage::Typical(p) => {
                data_array.sample_typical(None, *p, min_keep);
            }
            SamplerStage::TailFree(z) => data_array.sample_tail_free(None, *z, min_keep),
            SamplerStage::Dry { .. } => {
                sample_dry(context, &mut data_array, tokens, rep_pen_range, self)
            }
            SamplerStage::Xtc {
                probability: prob,
                threshold,
            } => sample_xtc(&mut data_array, *prob, *threshold),
        }

        Ok(data_array)
    }
}

impl Sampler {
    pub fn sample(
        &self,
        context: &mut LlamaContext,
        mut data_array: LlamaTokenDataArray,
        tokens: &[LlamaToken],
    ) -> Result<LlamaToken> {
        for stage in &self.stages {
            data_array = stage.apply(
                self.rep_pen_range,
                self.min_keep,
                context,
                data_array,
                tokens,
            )?;
        }

        Ok(data_array.sample_token(context))
    }
}
