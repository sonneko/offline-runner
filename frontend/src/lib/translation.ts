import { pipeline } from '@xenova/transformers';

export class TranslationService {
    private translator: any = null;
    private initialized = false;

    async init() {
        if (this.initialized) return;
        try {
            // We'll use a very small model for testing if possible,
            // or just provide the logic for it.
            this.translator = await pipeline('translation', 'Xenova/m2m100_418M');
            this.initialized = true;
        } catch (e) {
            console.error("Failed to initialize translator:", e);
        }
    }

    async translate(text: string, targetLang: string) {
        await this.init();
        if (!this.translator) {
             return `[Mock] ${text} -> ${targetLang}`;
        }

        // Map targetLang to NLLB/M2M codes if necessary
        const output = await this.translator(text, {
            tgt_lang: targetLang,
        });
        return output[0].translation_text;
    }
}
