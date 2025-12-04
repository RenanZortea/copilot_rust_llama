import os
import io
import uvicorn
import soundfile as sf
import torch
import sys
import numpy as np
from fastapi import FastAPI, Response
from pydantic import BaseModel
from contextlib import asynccontextmanager

# Add VibeVoice to path
sys.path.append(os.getcwd())

# Global Model
model = None

@asynccontextmanager
async def lifespan(app: FastAPI):
    global model
    try:
        print("--- LOADING VIBEVOICE MODEL (DOCKER) ---")
        from vibevoice import VibeVoice
        
        # Determine device
        device = "cuda" if torch.cuda.is_available() else "cpu"
        print(f"Using device: {device}")
        
        # Load the Realtime 0.5B model
        model = VibeVoice.from_pretrained("microsoft/VibeVoice-Realtime-0.5B")
        if device == "cuda":
            model = model.cuda()
            
        print("--- VOICE MODEL LOADED SUCCESSFULLY ---")
    except Exception as e:
        print(f"\nCRITICAL ERROR LOADING MODEL: {e}")
        print("Ensure container has internet access to download weights.\n")
    yield

app = FastAPI(lifespan=lifespan)

class TTSRequest(BaseModel):
    text: str

@app.post("/tts")
async def generate_speech(req: TTSRequest):
    global model
    sr = 24000
    
    if not model:
        print(f"Mocking: {req.text}")
        audio = np.zeros(sr, dtype=np.float32)
    else:
        print(f"Generating: {req.text}")
        try:
            output = model.generate(req.text)
            
            if isinstance(output, tuple):
                audio = output[0]
                sr = output[1]
            else:
                audio = output
                
            if hasattr(audio, 'cpu'):
                audio = audio.cpu().float().numpy()
            
            if len(audio.shape) > 1:
                audio = audio.flatten()
                
        except Exception as e:
            print(f"Generation Error: {e}")
            audio = np.zeros(sr, dtype=np.float32)

    buffer = io.BytesIO()
    sf.write(buffer, audio, sr, format='WAV')
    buffer.seek(0)
    
    return Response(content=buffer.read(), media_type="audio/wav")

if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=5000)
