# Prompted LLM Steganography
This project hides and recovers secret messages within AI-generated text. For text messages (the only kind supported at the moment), it is highly efficient, able to encode a message in only three to ten times the amount of space while still appearing as normal AI-generated text. It takes advantage of LLMs' natural ability to predict a probability distribution both to compress the secret message so it can be encoded in less text and to hide information in the wording and formatting of a text document. A simple explanation of how it encodes information in text is that it uses the secret message as the source of randomness to select the next token, and recovers the message by reverse-engineering what that randomness must have been.

## Usage
Encoding a message
```bash
echo 'Hello, World!' | cargo r -r -- --model /path/to/model encode 'Write a paragraph explaning the origins of the term "Hello, World!".' | tee hello_world.txt
```

Decoding the message
```bash
cat hello_world.txt | cargo r -r -- --model /path/to/model decode | tee decoded.txt
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
The core concept used in this application is [range coding](https://en.wikipedia.org/wiki/Range_coding). Both when compressing and encoding text, the probability distribution over tokens produced by the language model is used as the probability distribution of the range coder. While the compression uses this distribution directly, the encoding takes extra steps to ensure that the output is conditioned by the prompt.

The encoder uses two contexts: one with access to the prompt, the "writer", and one without, the "steganographer". The writer greedily selects the first few tokens, 8 by default, to steer the generation in the right direction. From then on, the steganographer filters for only the most likely tokens, filtering out all tokens that are less than 2% the probability of the most likely token. It then selects one of the filtered tokens using range coding. Finally, the writer chooses the most likely token among the tokens that were filtered out and the token that the steganographer selected, adding it to the genereated text.

Decoding the message only requires one context. The first few tokens are ignored, as none of the message was encoded into them. Each other token is checked for whether it is within the filter. If it is, it's range among the filtered tokens is fed to the range coder. Otherwise, it is ignored and not used in range coding.
