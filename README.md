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
cat src/decoder.rs | cargo r -r -- --model Meta-Llama-3.1-8B-Instruct-Q5_K_M.gguf encode 'Write a detailed survey of techniques one might use to implement a grabage collector in a programming language.' | tee garbage_collector.txt
```

Command to decode:
```bash
cat garbage_collector.txt | cargo r -r -- --model Meta-Llama-3.1-8B-Instruct-Q5_K_M.gguf decode | tee decoded_decoder.rs
```

The message:
```
Implementing a garbage collector in a programming language typically requires consideration of various techniques, ranging from fundamental collection algorithms to specific data structure approaches for the heap, and various techniques to facilitate memory allocation and deallocation. A thorough survey of possible implementations can be tailored to specific performance and memory management goals. Below are some key methodologies one should consider when designing and implementing a garbage collector for a programming language.

## Basic Garbage Collection Techniques
1. **Mark and Sweep**: This approach marks reachable objects and then sweeps through memory to free unmarked ones. It's simple but has a high pause time.

2. **Copy Collection**: A newer approach where all reachable objects are copied to a new heap. The old heap is then freed, avoiding the pause time issue but potentially causing fragmentation.

3. **Generational Collection**: Objects are divided into generations based on their lifetime. This strategy reduces the number of objects that need to be collected and the frequency with which it occurs.

4. **Concurrent Mark and Sweep**: Runs the garbage collection concurrently with the application, minimizing pause times. It's complex but can significantly improve responsiveness.

## Heap Management
1. **Heap Allocation**: Efficient allocation mechanisms are critical for garbage collectors. Techniques include:
   - **First-fit**: The first large enough free block is allocated.
   - **Best-fit**: The smallest large enough free block is allocated.
   - **Worst-fit**: The largest free block is allocated.
   - **Buddy Allocation**: Blocks are divided into pairs (buddies) to simplify memory management.

2. **Heap Deallocation**: Efficient deallocation is also crucial. Garbage collectors may use simple strategies like placing freed objects back into the free list or more complex ones like maintaining a separate free list for each object size.

## Memory Management and Allocation
1. **Reference Counting**: Each object has a reference count that increments and decrements as objects are allocated or deallocated. When a count drops to zero, the object is freed.

2. **Epoch-based Allocation**: Objects are assigned a timestamp (epoch) when they are allocated. The garbage collector then identifies objects that have survived without being referenced since the epoch was assigned.

3. **Profiling and Feedback**: To optimize garbage collection performance, it's essential to provide profiling data and feedback mechanisms to the garbage collector about object lifetimes and usage patterns.

## Hybrid and Incremental Collection
1. **Incremental Collection**: The garbage collector stops the application briefly for a small amount of work, then resumes it, repeating this process until the collection is complete.

2. **Hybrid Collection**: A combination of different collection strategies, for example, using concurrent mark and sweep for young generations but stop-the-world collection for old generations.

## Finalizers and Deallocation
1. **Finalizers**: Allow objects to perform clean-up actions before they're garbage collected.

2. **Deallocation Algorithms**: Efficiently deallocate memory to reduce memory fragmentation and improve garbage collector performance.

Implementing a garbage collector requires a deep understanding of memory management, algorithmic complexity, and the trade-offs between pause time, memory usage, and execution speed. Tailoring a garbage collection strategy to the specific requirements and constraints of the programming language, such as performance goals, object lifetime patterns, and memory availability,
```
