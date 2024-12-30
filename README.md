# Prompted LLM Steganography
This project hides and recovers secret messages within AI-generated text. For text messages (the only kind supported at the moment), it is highly efficient, able to encode a message in only three to ten times the amount of space while still appearing as normal AI-generated text. It takes advantage of LLMs' natural ability to predict a probability distribution both to compress the secret message so it can be encoded in less text and to hide information in the wording and formatting of a text document. A simple explanation of how it encodes information in text is that it uses the secret message as the source of randomness to select the next token, and recovers the message by reverse-engineering what that randomness must have been.

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
The core concept used in this application is [range coding](https://en.wikipedia.org/wiki/Range_coding). Both when compressing and encoding text, the probability distribution over tokens produced by the language model is used as the probability distribution of the range coder. While the compression uses this distribution directly, the encoding takes extra steps to ensure that the output is conditioned by the prompt.

The encoder uses two contexts: one with access to the prompt, the "writer", and one without, the "steganographer". The writer greedily selects the first few tokens, 8 by default, to steer the generation in the right direction. From then on, the steganographer filters for only the most likely tokens, filtering out all tokens that are less than 2% the probability of the most likely token. It then selects one of the filtered tokens using range coding. Finally, the writer chooses the most likely token among the tokens that were filtered out and the token that the steganographer selected, adding it to the genereated text.

Decoding the message only requires one context. The first few tokens are ignored, as none of the message was encoded into them. Each other token is checked for whether it is within the filter. If it is, it's range among the filtered tokens is fed to the range coder. Otherwise, it is ignored and not used in range coding.

## Example message
Resources used:
- [Bartowski's](https://huggingface.co/bartowski) quantization of [Meta Llama 3.1 8B Instruct](https://huggingface.co/meta-llama/Llama-3.1-8B-Instruct), found [here](https://huggingface.co/bartowski/Meta-Llama-3.1-8B-Instruct-GGUF/blob/main/Meta-Llama-3.1-8B-Instruct-Q5_K_M.gguf).
- [decoder.rs](https://github.com/nkoppel/llama-cpp-steganography/blob/f6e23feaa1cb9f6d708884eeefc8bcbc36f716de/src/decoder.rs), a part of this project's source code.

Command to encode:
```bash
cat src/decoder.rs | cargo r -r -- --model Meta-Llama-3.1-8B-Instruct-Q5_K_M.gguf encode 'Explain how UTF-8 works in detail.' | tee encoded_decoder.txt
```

Command to decode:
```bash
cat encoded_decoder.txt | cargo r -r -- --model Meta-Llama-3.1-8B-Instruct-Q5_K_M.gguf decode | tee decoded_decoder.rs
```

The message:
```
UTF-8 (8-bit Unicode Transformation Format-8)
=============================================

UTF-8 is a character encoding that is commonly used on the Internet. It is an extension of the ASCII character encoding standard but capable of representing characters from any Unicode-supported language. UTF-8 was designed to be compatible with ASCII and to avoid the need for explicit encoding declarations.

### UTF-8 Basics

Here are the main characteristics of UTF-8:

*   **Variable-length encoding:** UTF-8 uses one to four bytes to represent a character.
*   **Backward compatibility:** UTF-8 preserves the ASCII encoding, which means that all ASCII characters are represented using the same code points (values).
*   **Multi-byte sequences:** UTF-8 uses multi-byte sequences to represent non-ASCII characters, which means that a sequence of bytes can represent a single Unicode character.
*   **BOM (Byte Order Mark):** UTF-8 files do not use a BOM (Byte Order Mark) by default, but you can use a BOM if you want to.

### UTF-8 Character Encoding

UTF-8 uses a binary structure known as an "UTF-8 sequence" to represent characters. These sequences are designed to be more efficient to process than the variable-length representations of other Unicode encoding forms.

An example of how characters can be represented in UTF-8 is as follows:

| Byte Sequence | Unicode Code Point | Character |
| --- | --- | --- |
| `0x00` | U+0000 | Null character |
| `0x01` | U+0001 | Start of heading |
| `0x02` | U+0002 | Start of text |
| `0x03` | U+0003 | End of text |
| `0x04` | U+0004 | End of selection |
| `0x05` | U+0005 | Cancel |
| `0x06` | U+0006 | Start of highlighted text |
| `0x07` | U+0007 | Start of graphics |
| `0x08` | U+0008 | End of graphics |
| ... | ... | ... |

### UTF-8 Structure

A UTF-8 sequence consists of one or more bytes that start with a byte value between 0x00 and 0x7F. Characters are encoded as follows:

*   **ASCII-compatible characters:** These are characters with Unicode code points less than U+0080 and are encoded using a single byte equal to the code point.
*   **Non-ASCII characters:** These are characters with Unicode code points greater than or equal to U+0080, but less than U+0800. These are encoded using two bytes with the first byte being `0xC2` followed by a byte with the code point shifted by 6 bits.
*   **Supplementary characters:** These are characters with Unicode code points greater than or equal to U+0800 but less than U+10000. These are encoded using three bytes with the first byte being `0xE0` followed by two bytes that have code points shifted by 6 bits.

Here is the structure of the UTF-8 sequence:

*   **Byte 1:** The first byte of a UTF-8 sequence is in the range of 0xC2 to 0xF4 and represents a code point with a range of `U+0080` to `U+10FFFF`.
*   **Byte 2 to N:** The following bytes of the sequence, if any, are in the range of `U+80` to `U+BF` and represent a code point with a range of `U+0800` to `U+10FFFF`.

### UTF-8 Conversion Examples

Here are some examples of how to encode and decode UTF-8 strings:

*   **Single ASCII character:** `hello` -> `\x68\x65\x6c\x6c\x6f`
*   **Single non-ASCII character:** `¡` -> `\xc2\xa1`
*   **Supplementary character:** `€` -> `\xe2\x82\xac`

### UTF-8 Parsing and Processing

You can parse and process UTF-8 strings by using a library that supports UTF-8 encoding. Here are some steps for UTF-8 parsing:

1.  **Read a byte:** Read the first byte of the sequence. This byte should be in the range `0x00` to `0x7F` for ASCII characters or in the range `0xC2` to `0xF4` for non-ASCII characters.
2.  **Determine the length of the sequence:** For ASCII characters, the length of the sequence is one. For non-ASCII characters, the length of the sequence can be two, three, or four bytes.
3.  **Shift the code point:** For non-ASCII characters, shift the code point by 6 bits for each byte after the first byte.
```
