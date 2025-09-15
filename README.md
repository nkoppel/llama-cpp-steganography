# Prompted LLM Steganography
This project hides and recovers secret messages within AI-generated text. It takes advantage of the LLMs' natural ability to predict probability distributions over text to both compress and encode secret messages in a way that appears like normal AI-generated text.

Key features:

* **Recover messages without the prompt:** You do not need to have access to the prompt you used to generate the text in order to recover the secret message.
* **Text compression:** The language model is used to compress the hidden message, allowing it to be hidden in less text. This compression means that you'll usually need 3-10 times the text to encode a text message.
* **Difficult to detect:** The generated text looks like normal AI text, and should be very difficult to detect, as the compression step gets the message very close to random noise. Default settings may allow undesirable tokens to be generated at times, but these settings can be tweaked at the cost of encoding efficiency.

## Usage
Encoding a message
```bash
echo 'Hello, World!' | cargo r -r -- --model /path/to/model.gguf encode 'Write a paragraph explaning the origins of the term "Hello, World!".' | tee hello_world.txt
```

Decoding the message
```bash
cat hello_world.txt | cargo r -r -- --model /path/to/model.gguf decode | tee decoded.txt
```

See the help text for more information:
```bash
cargo r -r -- -h
```
```bash
cargo r -r -- encode -h
```
```bash
cargo r -r -- decode -h
```

IMPORTANT: Note that sampler settings and the model file must be exactly the same in order to decode successfully, or the small differences in the probability distributions produced will corrupt the message completely. Changing from CPU to GPU, from one GPU to another, or from one CPU to another may also exist, but I have not tested this. If hardware does make a difference, this is an issue upstream in [llama.cpp](https://github.com/ggerganov/llama.cpp) and there is nothing this application can do to fix it.

## Technical Explanation
The core concept used in this application is [range coding](https://en.wikipedia.org/wiki/Range_coding). Range encoding takes a sequence of symbols and their probability distribution at each step and compresses them to a stream of bits, and range decoding takes those bits and recovers the symbols. In this application's case, the symbols are the language model's tokens, and the probabilities are taken from the language model's output. This is the exact process used to compress and decompress the hidden message, while the process to hide it in the generated text is more complicated.

Two generation contexts are used to produce the encoded text:
* The writer: Has access to the user's prompt, and tries to influence generation in accordance with the prompt
* The steganographer: Does not have access to the user's prompt, and encodes the message in a manner that the decoder can understand.

If we are generating the first few tokens, 8 by default, the writer chooses whichever token it thinks is most likely. This makes sure that some information about the prompt is available to the writer before we start using it. To generate a token after this point:
1. The steganographer filters for some number of the most likely tokens from its perspective. By default, tokens with probability greater than 2% the probability of the most likely token are allowed through.
2. The steganographer uses a range decoder that is decoding the compressed message to select a token from among the filtered tokens.
3. The writer observes the probability of the selected token and all tokens that were filtered out in step 1, and chooses whichever token it thinks is most likely.
4. If the writer and steganographer have selected the same token, the range decoder is updated to reflect the fact that some of the message has been encoded into the text.
5. The writer and staganographer both add the token the writer selected to their prompts.

Once the range decoder has finished decoding the compressed message, tokens can be generated in an arbitrary fashion. This implementation simply has the writer select the most likely token. Decoding works much the same way as encoding, from the perspective of the steganographer:
1. The decoding context filters the tokens in the same manner as the steganographer.
2. If the actual token is within the filter, it is encoded into a range encoder. Otherwise, it is ignored.
3. Once the decoder has read the entire generation, the range encoder contains the compressed message, which is decompressed and displayed to the user.

## Example message
Resources used:
- [Unsloth's](https://huggingface.co/unsloth) quantization of [Meta Llama 3.1 8B Instruct](https://huggingface.co/meta-llama/Llama-3.1-8B-Instruct), found [here](https://huggingface.co/unsloth/Llama-3.1-8B-Instruct-GGUF/blob/main/Llama-3.1-8B-Instruct-UD-Q4_K_XL.gguf).
- [from\_utf8\_lossy\_inplace.rs](from_utf8_lossy_inplace.rs), a part of this project's source code.

Command to encode:
```bash
cat from_utf8_lossy_inplace.rs | cargo r -r -- --model Meta-Llama-3.1-8B-Instruct-Q5_K_M.gguf encode 'Write a long, detailed essay arguing that UTF-8 is an effecient, effective format for international digital communications.' | tee from_utf8_lossy_inplace_encoded.md
```

Command to decode:
```bash
cat from_utf8_lossy_inplace_encoded.md | cargo r -r -- --model Meta-Llama-3.1-8B-Instruct-Q5_K_M.gguf decode | tee from_utf8_lossy_inplace_decoded.rs
```

The encoded message can be found in [from\_utf8\_lossy\_inplace\_encoded.md](from_utf8_lossy_inplace_encoded.md).
