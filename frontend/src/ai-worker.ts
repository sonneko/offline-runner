import { pipeline, env } from '@xenova/transformers';

// Setup caching and wasm paths
env.allowLocalModels = false;
env.useBrowserCache = true;

let translator: any = null;

self.onmessage = async (e) => {
    const { id, type, text, targetLang } = e.data;

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
                    self.postMessage({ id, type: 'progress', message: `Downloading model: ${x.file || '...'} (${Math.round(x.progress || 0)}%)` });
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
    }
};
