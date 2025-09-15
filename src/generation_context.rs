use std::{borrow::Cow, sync::atomic::AtomicBool};

use crate::decoder::TokenDecoder;
use anyhow::{Context, Result};
use llama_cpp_2::{
    context::{params::LlamaContextParams, LlamaContext},
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{AddBos, LlamaModel, Special},
    sampling::LlamaSampler,
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

pub trait LanguageModel {
    fn partial_clone(&self) -> Result<Self>
    where
        Self: Sized;
    fn add_tokens(&mut self, tokens: &[LlamaToken]) -> Result<()>;
    fn truncate_tokens(&mut self, length: usize) -> Result<()>;

    fn tokens(&self) -> &[LlamaToken];
    fn get_token_data(&self) -> LlamaTokenDataArray;
    fn model(&self) -> &LlamaModel;

    fn tokenize(&self, text: &str) -> Result<Vec<LlamaToken>> {
        let mut out = self.model().str_to_token(text, AddBos::Never)?;
        out.insert(0, self.model().token_bos());
        Ok(out)
    }
    fn detokenize(&self, tokens: &[LlamaToken]) -> Result<String> {
        let bytes = tokens
            .iter()
            .flat_map(|token| self.model().token_to_bytes(*token, Special::Tokenize))
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

    fn get_prompt(&self) -> Result<String> {
        self.detokenize(self.tokens())
    }
    fn set_tokens(&mut self, new_tokens: &[LlamaToken]) -> Result<()> {
        let n_tokens = {
            let i = self
                .tokens()
                .iter()
                .zip(new_tokens)
                .position(|(t1, t2)| t1.0 != t2.0)
                .unwrap_or(self.tokens().len().min(new_tokens.len()));

            if i == new_tokens.len() && i < self.tokens().len() {
                new_tokens.len().saturating_sub(1)
            } else {
                i
            }
        };
        self.truncate_tokens(n_tokens)?;
        self.add_tokens(&new_tokens[n_tokens..])
    }
    fn add_tokens_get_token_data(
        &mut self,
        tokens: &[LlamaToken],
    ) -> Result<Vec<LlamaTokenDataArray>> {
        let mut out = vec![self.get_token_data()];

        for token in tokens {
            self.add_token(*token)?;
            out.push(self.get_token_data());
        }

        Ok(out)
    }
    fn set_prompt(&mut self, prompt: &str) -> Result<()> {
        self.set_tokens(&self.tokenize(prompt)?)
    }
    fn add_token(&mut self, token: LlamaToken) -> Result<()> {
        self.add_tokens(&[token])
    }
    fn sample_token(&mut self, sampler: &mut LlamaSampler) -> Result<LlamaToken> {
        let mut data_array = self.get_token_data();
        sampler.apply(&mut data_array);
        data_array
            .selected_token()
            .context("Sampler did not select a token.")
    }
    fn clear(&mut self) -> Result<()> {
        self.set_prompt("")
    }
    fn generate_token(&mut self, sampler: &mut LlamaSampler) -> Result<LlamaToken> {
        let token = self.sample_token(sampler)?;
        self.add_token(token)?;
        sampler.accept(token);
        Ok(token)
    }
    fn iter_generated_tokens(
        &mut self,
        sampler: &mut LlamaSampler,
    ) -> impl Iterator<Item = Result<LlamaToken>> {
        std::iter::from_fn(|| match self.generate_token(sampler) {
            Ok(token) => (!self.model().is_eog_token(token)).then_some(Ok(token)),
            Err(e) => Some(Err(e)),
        })
    }
}

impl LanguageModel for GenerationContext<'_> {
    fn partial_clone(&self) -> Result<Self> {
        Self::new(self.context.model, self.params().clone())
    }

    fn add_tokens(&mut self, tokens: &[LlamaToken]) -> Result<()> {
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

    fn truncate_tokens(&mut self, n_tokens: usize) -> Result<()> {
        self.context
            .clear_kv_cache_seq(Some(0), Some(n_tokens as u32), None)?;
        self.tokens.truncate(n_tokens);

        Ok(())
    }

    fn tokens(&self) -> &[LlamaToken] {
        &self.tokens
    }

    fn get_token_data(&self) -> LlamaTokenDataArray {
        LlamaTokenDataArray::from_iter(
            self.context.candidates_ith(self.batch.n_tokens() - 1),
            false,
        )
    }

    fn model(&self) -> &LlamaModel {
        self.context.model
    }
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

    pub fn add_tokens_get_token_data(
        &mut self,
        tokens: &[LlamaToken],
    ) -> Result<Vec<LlamaTokenDataArray>> {
        let mut out = vec![self.get_token_data()];

        for token in tokens {
            self.add_token(*token)?;
            out.push(self.get_token_data());
        }

        Ok(out)
    }

    pub fn embeddings(&self) -> Result<&[f32]> {
        self.context
            .embeddings_ith(self.batch.n_tokens() - 1)
            .map_err(Into::into)
    }

    pub fn params(&self) -> &LlamaContextParams {
        &self.params
    }

    pub fn context(&self) -> &LlamaContext {
        &self.context
    }

    pub fn model_longlived(&self) -> &'a LlamaModel {
        self.context.model
    }
}

pub fn generate_tokens(
    model: &LlamaModel,
    tokens: impl Iterator<Item = Result<LlamaToken>>,
) -> Result<Vec<LlamaToken>> {
    tokens
        .take_while(|t| t.as_ref().map_or(true, |t| !model.is_eog_token(*t)))
        .collect()
}

pub fn generate_text(
    model: &LlamaModel,
    preview: bool,
    tokens: impl Iterator<Item = Result<LlamaToken>>,
) -> Result<String> {
    let mut token_decoder = TokenDecoder::new();

    let tokens = tokens.map(|t| {
        let Ok(t) = t else {
            return t;
        };

        let piece = token_decoder.add_token(&model.token_to_bytes(t, Special::Tokenize)?);

        if preview {
            print!("{piece}");
            std::io::Write::flush(&mut std::io::stdout())?;
        }

        Ok(t)
    });

    generate_tokens(model, tokens)?;

    if preview {
        print!("{}", token_decoder.last_part());
    }

    Ok(token_decoder.into_string())
}
