import mermaid from 'mermaid';

self.onmessage = async (e) => {
    const { id, content, theme } = e.data;
    try {
        mermaid.initialize({ startOnLoad: false, theme: theme || 'default' });
        const { svg } = await mermaid.render('mermaid-svg-' + id, content);
        self.postMessage({ id, svg, error: null });
    } catch (err: any) {
        self.postMessage({ id, svg: null, error: err.message || err.toString() });
    }
};
