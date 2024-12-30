#![allow(dead_code)]

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use generation_context::{get_backend, GenerationContext};
use llama_cpp_2::{
    context::params::LlamaContextParams,
    model::{params::LlamaModelParams, LlamaModel},
};

mod decoder;
mod generation_context;
mod logit_vector;
mod range_coder;
mod steganography;

#[derive(Parser, Debug)]
#[command(version, about)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// GGUF model file to use during inference
    #[arg(short, long)]
    model: String,

    /// Run inference on the GPU
    #[arg(long, short)]
    gpu: bool,

    /// File to use as input (defaults to stdin)
    #[arg(short, long)]
    infile: Option<String>,

    /// File to use as output (defaults to stdout)
    #[arg(short, long)]
    outfile: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Hide the message sent into stdin in the generated text
    Encode(EncodeArgs),

    /// Recover a hidden message from the text sent to stdin
    Decode(DecodeArgs),

    /// Get the length of the text sent to stdin before and after compression in bits.
    Compress,
}

#[derive(Args, Debug)]
#[command(version, about)]
struct EncodeArgs {
    /// The user prompt for the generated text
    prompt: String,

    /// The number of tokens to skip at the start of the generation before starting to encode the
    /// message
    #[arg(short = 'k', long, default_value_t = 8)]
    skip_start: usize,

    /// The maximum number of tokens to generate
    #[arg(short, long, default_value_t = 1024)]
    token_count: usize,

    /// MinP filtering value for sampling
    #[arg(long, default_value_t = 0.02)]
    min_p: f32,

    /// TopK filtering value for sampling
    #[arg(long, default_value_t = 0)]
    top_k: usize,

    /// Temperature sampling value
    #[arg(long, default_value_t = 1.0)]
    temp: f32,
}

#[derive(Args, Debug)]
#[command(version, about)]
struct DecodeArgs {
    /// The number of tokens to skip at the start of the generation before starting to encode the
    /// message
    #[arg(short = 'k', long, default_value_t = 8)]
    skip_start: usize,

    /// MinP filtering value for sampling
    #[arg(long, default_value_t = 0.02)]
    min_p: f32,

    /// TopK filtering value for sampling
    #[arg(long, default_value_t = 0)]
    top_k: usize,

    /// Temperature sampling value
    #[arg(long, default_value_t = 1.0)]
    temp: f32,
}

impl EncodeArgs {
    fn as_decode_args(&self) -> DecodeArgs {
        DecodeArgs {
            skip_start: self.skip_start,
            min_p: self.min_p,
            top_k: self.top_k,
            temp: self.temp,
        }
    }
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let model = LlamaModel::load_from_file(
        get_backend()?,
        &args.model,
        &LlamaModelParams::default().with_n_gpu_layers(args.gpu as u32 * 1000),
    )?;
    let params = LlamaContextParams::default()
        .with_n_ctx(std::num::NonZeroU32::new(8192))
        .with_n_threads(4)
        .with_n_threads_batch(8)
        .with_n_batch(2048);

    let mut gen = GenerationContext::new(&model, params.clone())?;
    let input = std::io::read_to_string(std::io::stdin())?;

    match args.command {
        Command::Encode(encode_args) => {
            gen.encode_compressed(&input, &encode_args)?;
        }
        Command::Decode(decode_args) => {
            gen.decode_compressed(&input, &decode_args)?;
        }
        Command::Compress => {
            println!("Normal: {}", input.len() * 8);
            println!("Compressed: {}", gen.compress_message(&input)?.len());
        }
    }

    Ok(())
}
