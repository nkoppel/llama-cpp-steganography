FROM nvidia/cuda:12.6.2-base-ubuntu24.04
RUN apt-get update 

# Install Rust
RUN apt-get install -y curl
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Install Python and Gradio
RUN apt-get install -y python3 python3-pip
RUN pip install gradio --break-system-packages

# Copy and build rust app 
WORKDIR /app
COPY . .
RUN apt-get install -y clang cmake git
# RUN . "$HOME/.cargo/env" && cargo build --release --features cuda

# Download model
# ADD --checksum=sha256:2e998d7e181c8756c5ffc55231b9ee1cdc9d3acec4245d6e27d32bd8e738c474 https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q5_K_M.gguf /app/model/
