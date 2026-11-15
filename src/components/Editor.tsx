import { useEffect, useRef } from 'react';
import { basicSetup } from 'codemirror';
import { indentUnit, syntaxHighlighting, HighlightStyle } from '@codemirror/language';
import { EditorState, Prec } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import { searchKeymap } from '@codemirror/search';
import { tags } from '@lezer/highlight';
import { plainLanguage } from '../lib/plainLanguage';

const plainHighlight = HighlightStyle.define([
  { tag: tags.keyword, color: '#5ed8bf', fontWeight: '700' },
  { tag: tags.typeName, color: '#d5a867' },
  { tag: tags.string, color: '#c7d98a' },
  { tag: tags.number, color: '#d89976' },
  { tag: tags.comment, color: '#718790', fontStyle: 'italic' },
  { tag: tags.operator, color: '#a8bac2' },
  { tag: tags.variableName, color: '#d7e0e3' },
]);

const editorTheme = EditorView.theme({
  '&': { height: '100%', backgroundColor: '#111a20', color: '#d9e2e5', fontSize: '13px' },
  '.cm-scroller': { fontFamily: '"IBM Plex Mono", "SFMono-Regular", Consolas, monospace', overflow: 'auto', lineHeight: '1.75' },
  '.cm-content': { padding: '18px 0', caretColor: '#39d4b5' },
  '.cm-gutters': { backgroundColor: '#111a20', color: '#51616a', border: 'none', paddingLeft: '10px' },
  '.cm-activeLineGutter': { color: '#c4d1d5', backgroundColor: '#17242a' },
  '.cm-activeLine': { backgroundColor: '#17242a66' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: '#39d4b5' },
  '.cm-selectionBackground, ::selection': { backgroundColor: '#235e5a !important' },
  '.cm-search': { backgroundColor: '#142129', color: '#e7efef', padding: '8px' },
  '.cm-search input': { backgroundColor: '#0d1419', color: '#edf3f2', border: '1px solid #30414a', borderRadius: '4px' },
  '.cm-search button': { color: '#c9d3d5', background: '#203039', border: '1px solid #344851', borderRadius: '4px' },
}, { dark: true });

interface Props {
  path: string;
  value: string;
  onChange: (value: string) => void;
  onSave: () => void;
  onRun: () => void;
  onCursor: (line: number, column: number) => void;
  onView: (view: EditorView | null) => void;
}

export function Editor({ path, value, onChange, onSave, onRun, onCursor, onView }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const callbacks = useRef({ onChange, onSave, onRun, onCursor, onView });
  callbacks.current = { onChange, onSave, onRun, onCursor, onView };

  useEffect(() => {
    if (!host.current) return undefined;
    const state = EditorState.create({
      doc: value,
      extensions: [
        basicSetup,
        indentUnit.of('  '),
        plainLanguage,
        syntaxHighlighting(plainHighlight),
        editorTheme,
        EditorView.lineWrapping,
        Prec.highest(keymap.of([
          { key: 'Mod-s', run: () => { callbacks.current.onSave(); return true; } },
          { key: 'Mod-Enter', run: () => { callbacks.current.onRun(); return true; } },
          ...searchKeymap,
        ])),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) callbacks.current.onChange(update.state.doc.toString());
          if (update.docChanged || update.selectionSet) {
            const position = update.state.doc.lineAt(update.state.selection.main.head);
            callbacks.current.onCursor(position.number, update.state.selection.main.head - position.from + 1);
          }
        }),
      ],
    });
    const view = new EditorView({ state, parent: host.current });
    callbacks.current.onView(view);
    return () => { callbacks.current.onView(null); view.destroy(); };
  }, [path]);

  return <div className="editor-host" ref={host} aria-label={`Plain code editor: ${path}`} />;
}
