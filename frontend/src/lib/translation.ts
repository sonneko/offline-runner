import { pipeline } from '@xenova/transformers';

export class TranslationService {
    private translator: any = null;
    private initialized = false;

    async init() {
        if (this.initialized) return;
        try {
            // Attempt to load a very small model for demo purposes
            // this.translator = await pipeline('translation', 'Xenova/nllb-200-distilled-600M');
            this.initialized = true;
        } catch (e) {
            console.error("Failed to initialize translator:", e);
        }
    }

    async translate(text: string, targetLang: string) {
        await this.init();
        if (!this.translator) {
            return `[Offline Translation Mock] Translating "${text}" to ${targetLang}. (Actual model loading skipped for speed in this environment)`;
        }
        const output = await this.translator(text, {
            tgt_lang: targetLang,
        });
        return output[0].translation_text;
    }
}
