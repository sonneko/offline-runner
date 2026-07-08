import { pipeline, env } from '@xenova/transformers';

// Setup caching and wasm paths
env.allowLocalModels = false;
env.useBrowserCache = true;

let translator: any = null;
let generator: any = null;

self.onmessage = async (e) => {
    const { id, type, text, targetLang, prompt } = e.data;

    if (type === 'init') {
        try {
            // Determine WebGPU support
            let device = 'wasm';
            if (navigator.gpu) {
                try {
                    const adapter = await navigator.gpu.requestAdapter();
                    if (adapter) {
                        device = 'webgpu';
                    }
                } catch (err) {}
            }

            self.postMessage({ id, type: 'progress', message: `Initializing translator (Device: ${device})...` });

            translator = await pipeline('translation', 'Xenova/nllb-200-distilled-600M', {
                device: device as any,
                progress_callback: (x: any) => {
                    self.postMessage({ id, type: 'progress', message: `Downloading translation model: ${x.file || '...'} (${Math.round(x.progress || 0)}%)` });
                }
            });

            generator = await pipeline('text-generation', 'Xenova/TinyLlama-1.1B-Chat-v1.0', {
                device: device as any,
                progress_callback: (x: any) => {
                    self.postMessage({ id, type: 'progress', message: `Downloading generation model: ${x.file || '...'} (${Math.round(x.progress || 0)}%)` });
                }
            });

            self.postMessage({ id, type: 'ready' });
        } catch (err: any) {
            self.postMessage({ id, type: 'error', error: err.message });
        }
    } else if (type === 'translate') {
        if (!translator) {
            self.postMessage({ id, type: 'error', error: 'Translator not initialized' });
            return;
        }
        try {
            const output = await translator(text, {
                tgt_lang: targetLang,
            });
            self.postMessage({ id, type: 'result', text: output[0].translation_text });
        } catch (err: any) {
             self.postMessage({ id, type: 'error', error: err.message });
        }
    } else if (type === 'generate') {
        if (!generator) {
            self.postMessage({ id, type: 'error', error: 'Generator not initialized' });
            return;
        }
        try {
            const systemPrompt = "You are an assistant that outputs valid Mini-ShellScript (MSS) code based on user requests. Only output code, no explanations.";
            const formattedPrompt = `<|system|>\n${systemPrompt}</s>\n<|user|>\n${prompt}</s>\n<|assistant|>\n`;

            const output = await generator(formattedPrompt, {
                max_new_tokens: 256,
                temperature: 0.7,
                do_sample: true
            });

            let generatedText = output[0].generated_text;
            // Extract only the assistant's response part
            const assistantMarker = "<|assistant|>\n";
            const markerIndex = generatedText.lastIndexOf(assistantMarker);
            if (markerIndex !== -1) {
                generatedText = generatedText.substring(markerIndex + assistantMarker.length);
            }

            self.postMessage({ id, type: 'result', text: generatedText.trim() });
        } catch (err: any) {
            self.postMessage({ id, type: 'error', error: err.message });
        }
    }
};
