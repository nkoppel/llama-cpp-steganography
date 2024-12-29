import gradio as gr
import subprocess

yoga_exmaple = '''Yoga is a physical, mental, and spiritual practice or tradition that has its origins in ancient India. The word 'yoga' derives from Sanskrit, and its primary meanings are 'union' or 'to unite'.

Yoga typically consists of various components:

1. **Physical Exercises (Asanas)**: Involves the use of different postures and movements to stretch and strengthen the body.
2. **Breathing Techniques (Pranayama)**: Focuses on the regulation of breath to enhance the flow of energy in the body.
3. **Meditation (Dhyana)**: Involves techniques to quiet the mind and achieve a state of concentration or awareness.
4. **Philosophical Teachings**: Often includes the study of ancient texts and the practice of meditation to gain deeper insight into the nature of reality.

Yoga is practiced for a variety of reasons, including physical fitness, stress relief, mental clarity, and spiritual growth. There are many different styles and forms of yoga, each with its own unique focus and approach. Some popular styles include Hatha, Vinyasa, Ashtanga, Iyengar, and Kundalini yoga.'''

def encode(prompt, message, token_count, skip, min_p, top_k, temp):
    res = subprocess.run([
        "target/release/llama_testing",
        "--model", "/home/nathan/software/models/josiefied-qwen2.5-7b-instruct-abliterated-v2.Q5_K_M.gguf",
        "encode",
        "--token-count", str(token_count),
        "--skip-start", str(skip),
        "--min-p", str(min_p),
        "--top-k", str(top_k),
        "--temp", str(temp),
        prompt,
    ], input=message.encode('utf-8'), capture_output=True)

    if res.returncode == 0:
        return res.stdout.decode('utf-8')
    else:
        return res.stderr.decode('utf-8')

def decode(prompt, skip, min_p, top_k, temp):
    res = subprocess.run([
        "target/release/llama_testing",
        "--model", "/home/nathan/software/models/josiefied-qwen2.5-7b-instruct-abliterated-v2.Q5_K_M.gguf",
        "decode",
        "--skip-start", str(skip),
        "--min-p", str(min_p),
        "--top-k", str(top_k),
        "--temp", str(temp),
    ], input=prompt.encode('utf-8'), capture_output=True)

    if res.returncode == 0:
        return res.stdout.decode('utf-8')
    else:
        return res.stderr.decode('utf-8')

with gr.Blocks() as demo:
    gr.Markdown("# Prompt-free LLM Stenography with Compression")
    gr.Markdown("This allows you to hide a secret message in a prompted piece of LLM-generated text and recover it at a later date. Since the message is stored in the output text, you will need to provide a prompt that generates a text long enough to fit your message or the application will error. This application compresses the text before it is encoded, which means that it only needs a generation 5-10 times as long as the secret for the default settings.")
    gr.Markdown("Be careful when changing the sampler settings to make the output more predictable, as the more predictable the output is, the less information the encoder can store per token. Also be careful not to reduce the token filtering (MinP and TopK) too much, as these allow the prompt to influence the output after the initial greedy tokens.")
    gr.Markdown("**IMPORTANT:** When decoding, be sure that the sampler settings are *exactly* the same as when encoding, or the output will be corrupted.")

    with gr.Tab("Encode Message"):
        with gr.Row():
            with gr.Column():
                prompt_input = gr.Textbox("What is yoga?", label="Prompt", show_copy_button=True)
                message_input = gr.Textbox("This is a secret.", label="Secret Message", show_copy_button=True)
                token_count_slider = gr.Slider(value=1024, minimum=256, maximum=2048, step=256, label="Maximum Tokens")
                skip_slider = gr.Slider(value=8, minimum=0, maximum=64, step=8, label="Number of tokens to generate greedily at the start of the message")
                min_p_slider = gr.Slider(value=0.02, minimum=0, maximum=0.3, step=0.01, label="MinP sampling parameter")
                top_k_slider = gr.Slider(value=0, minimum=0, maximum=128, step=1, label="TopK sampling parameter")
                temp_slider = gr.Slider(value=1.0, minimum=0.2, maximum=2.0, step=0.1, label="Temperature sampling parameter")

            with gr.Column():
                output = gr.Textbox(yoga_exmaple, lines=20, max_lines=100, label="Output", show_copy_button=True)

        encode_button = gr.Button("Encode", variant="primary")
        gr.ClearButton(components=[prompt_input, message_input, output])

    encode_button.click(encode, inputs=[prompt_input, message_input, token_count_slider, skip_slider, min_p_slider, top_k_slider, temp_slider], outputs=output)

    with gr.Tab("Decode message"):
        with gr.Row():
            with gr.Column():
                prompt_input = gr.Textbox(yoga_exmaple, lines=20, max_lines=100, label="Prompt", show_copy_button=True)
                skip_slider = gr.Slider(value=8, minimum=0, maximum=64, step=8, label="Number of tokens that were generated greedily at the start of the message")
                min_p_slider = gr.Slider(value=0.02, minimum=0, maximum=0.3, step=0.01, label="MinP sampling parameter")
                top_k_slider = gr.Slider(value=0, minimum=0, maximum=128, step=1, label="TopK sampling parameter")
                temp_slider = gr.Slider(value=1.0, minimum=0.2, maximum=2.0, step=0.1, label="Temperature sampling parameter")

            with gr.Column():
                output = gr.Textbox("This is a secret.", lines=20, max_lines=100, label="Output", show_copy_button=True)

        decode_button = gr.Button("Decode", variant="primary")
        gr.ClearButton(components=[prompt_input, message_input, output])

    decode_button.click(decode, inputs=[prompt_input, skip_slider, min_p_slider, top_k_slider, temp_slider], outputs=output)

# demo.queue().launch(server_name="0.0.0.0")
demo.queue().launch()
