use std::{borrow::Cow, sync::atomic::AtomicBool};

use crate::sample::Sampler;
use anyhow::Result;
use llama_cpp_2::{
    context::{params::LlamaContextParams, LlamaContext},
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{AddBos, LlamaModel, Special},
    token::{data_array::LlamaTokenDataArray, LlamaToken},
};

pub struct GenerationContext<'a> {
    context: LlamaContext<'a>,
    tokens: Vec<LlamaToken>,
    batch: LlamaBatch,
    params: LlamaContextParams,
}

static BACKEND_SET: AtomicBool = AtomicBool::new(false);
static BACKEND: LlamaBackend = LlamaBackend {};

pub fn get_backend() -> Result<&'static LlamaBackend> {
    if !BACKEND_SET.swap(true, std::sync::atomic::Ordering::Relaxed) {
        LlamaBackend::init()?;
    }
    Ok(&BACKEND)
}

impl<'a> GenerationContext<'a> {
    pub fn new(model: &'a LlamaModel, params: LlamaContextParams) -> Result<Self> {
        let context = model.new_context(get_backend()?, params.clone())?;

        let mut out = Self {
            context,
            tokens: Vec::new(),
            batch: LlamaBatch::new(params.n_batch() as usize, 1),
            params,
        };
        out.set_prompt("")?;

        Ok(out)
    }

    pub fn partial_clone(&self) -> Result<Self> {
        Self::new(self.model(), self.params().clone())
    }

    pub fn add_tokens(&mut self, tokens: &[LlamaToken]) -> Result<()> {
        for chunk in tokens.chunks(self.params.n_batch() as usize) {
            self.batch.clear();
            for (i, token) in chunk.iter().enumerate() {
                self.batch
                    .add(*token, self.tokens.len() as i32, &[0], i == chunk.len() - 1)?;
                self.tokens.push(*token);
            }
            self.context.decode(&mut self.batch)?;
        }

        Ok(())
    }

    pub fn add_tokens_get_token_data(
        &mut self,
        tokens: &[LlamaToken],
    ) -> Result<Vec<LlamaTokenDataArray>> {
        let mut out = vec![self.get_token_data()];

        for token in tokens {
            self.add_token(*token)?;
            out.push(self.get_token_data());
        }

        // // More efficient implementation using batching, but this changes the output logits by a
        // // large enough margin to corrupt range coded messages.
        // for chunk in tokens.chunks(self.params.n_batch() as usize) {
        // self.batch.clear();
        // for token in chunk {
        // self.batch
        // .add(*token, self.tokens.len() as i32, &[0], true)?;
        // self.tokens.push(*token);
        // }
        // self.context.decode(&mut self.batch)?;

        // for i in 0..self.batch.n_tokens() {
        // out.push(LlamaTokenDataArray::from_iter(
        // self.context.candidates_ith(i),
        // false,
        // ));
        // }
        // }

        Ok(out)
    }

    pub fn set_tokens(&mut self, new_tokens: &[LlamaToken]) -> Result<()> {
        let n_tokens = {
            let i = self
                .tokens
                .iter()
                .zip(new_tokens)
                .position(|(t1, t2)| t1.0 != t2.0)
                .unwrap_or(self.tokens.len().min(new_tokens.len()));

            if i == new_tokens.len() && i < self.tokens.len() {
                new_tokens.len().saturating_sub(1)
            } else {
                i
            }
        };
        self.context
            .clear_kv_cache_seq(None, Some(n_tokens as u32), None)?;
        self.tokens.truncate(n_tokens);

        self.add_tokens(&new_tokens[n_tokens..])
    }

    pub fn set_prompt(&mut self, new_prompt: &str) -> Result<()> {
        self.set_tokens(&self.model().str_to_token(new_prompt, AddBos::Always)?)
    }

    pub fn clear(&mut self) -> Result<()> {
        self.set_prompt("")
    }

    pub fn add_token(&mut self, token: LlamaToken) -> Result<()> {
        self.add_tokens(&[token])
    }

    pub fn get_logits(&self) -> &[f32] {
        self.context.get_logits_ith(self.batch.n_tokens() - 1)
    }

    pub fn get_token_data(&self) -> LlamaTokenDataArray {
        LlamaTokenDataArray::from_iter(
            self.context.candidates_ith(self.batch.n_tokens() - 1),
            false,
        )
    }

    pub fn sample_with_token_data(
        &mut self,
        sampler: &Sampler,
        token_data: LlamaTokenDataArray,
    ) -> Result<LlamaToken> {
        sampler.sample(&mut self.context, token_data, &self.tokens)
    }

    pub fn sample_token(&mut self, sampler: &Sampler) -> Result<LlamaToken> {
        self.sample_with_token_data(sampler, self.get_token_data())
    }

    pub fn tokens(&self) -> &[LlamaToken] {
        &self.tokens
    }

    pub fn params(&self) -> &LlamaContextParams {
        &self.params
    }

    pub fn context(&self) -> &LlamaContext {
        &self.context
    }

    pub fn model(&self) -> &'a LlamaModel {
        self.context.model
    }

    pub fn detokenize(&self, tokens: &[LlamaToken]) -> Result<String> {
        let bytes = tokens
            .iter()
            .flat_map(|token| self.context.model.token_to_bytes(*token, Special::Tokenize))
            .flatten()
            .collect::<Vec<u8>>();

        if let Cow::Owned(string) = String::from_utf8_lossy(&bytes) {
            Ok(string)
        } else {
            // SAFETY: `String::from_utf8_lossy`'s contract ensures that if
            // it returns a `Cow::Borrowed`, it is a valid UTF-8 string.
            // Otherwise, it returns a new allocation of an owned `String`, with
            // replacement characters for invalid sequences, which is returned
            // above.
            unsafe { Ok(String::from_utf8_unchecked(bytes)) }
        }
    }

    pub fn get_prompt(&self) -> Result<String> {
        self.detokenize(&self.tokens)
    }
}
