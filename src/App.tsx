import "./App.css";
import { editor } from 'monaco-editor';
import { useEffect, useRef } from 'react';


function App() {
  const editorContainerRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    if (editorContainerRef.current !== null) {
      const codeEditor = editor.create(editorContainerRef.current, {
        value: `console.log('Hello World!')`,
        language: 'javascript',        
      });
      codeEditor.focus();
    }
  }, [editorContainerRef])

  return (
    <>
      <div className='editor-container' ref={editorContainerRef}></div>
    </>
  )
}

export default App;
