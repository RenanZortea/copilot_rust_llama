FROM python:3.11-slim

# Install system dependencies required for VibeVoice audio processing
RUN apt-get update && apt-get install -y \
    git \
    libsndfile1 \
    ffmpeg \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Clone the Community Fork (Better compatibility)
RUN git clone https://github.com/JarodMica/VibeVoice.git .

# Patch dependencies to remove flash-attn (prevents build failure on generic hardware)
# and relax python version constraints
RUN sed -i 's/==3.11.12/>=3.10/g' pyproject.toml && \
    sed -i '/flash-attn/d' pyproject.toml

# Install Python Dependencies
RUN pip install --no-cache-dir --upgrade pip setuptools wheel && \
    pip install --no-cache-dir "numpy<2" && \
    pip install --no-cache-dir fastapi uvicorn requests soundfile torch huggingface_hub

# Install the repo itself
RUN pip install -e .

# Copy our server script
COPY server.py .

# Expose the port
EXPOSE 5000

# Run the server
CMD ["python", "server.py"]
