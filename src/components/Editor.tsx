import { useEffect, useRef } from 'react';
import { basicSetup } from 'codemirror';
import { indentUnit, syntaxHighlighting, HighlightStyle } from '@codemirror/language';
import { EditorState, Prec } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import { searchKeymap } from '@codemirror/search';
import { tags } from '@lezer/highlight';
import { plainLanguage } from '../lib/plainLanguage';

const plainHighlight = HighlightStyle.define([
  { tag: tags.keyword, color: '#ff8a2a', fontWeight: '700' },
  { tag: tags.typeName, color: '#ffc078' },
  { tag: tags.string, color: '#e7c991' },
  { tag: tags.number, color: '#ffae75' },
  { tag: tags.comment, color: '#8d8076', fontStyle: 'italic' },
  { tag: tags.operator, color: '#c9bcb2' },
  { tag: tags.variableName, color: '#eee5dc' },
]);

const editorTheme = EditorView.theme({
  '&': { height: '100%', backgroundColor: '#101012', color: '#f1ece6', fontSize: '13px' },
  '.cm-scroller': { fontFamily: '"IBM Plex Mono", "SFMono-Regular", Consolas, monospace', overflow: 'auto', lineHeight: '1.75' },
  '.cm-content': { padding: '18px 0', caretColor: '#ff7a1a' },
  '.cm-gutters': { backgroundColor: '#101012', color: '#857b74', border: 'none', paddingLeft: '10px' },
  '.cm-activeLineGutter': { color: '#efe6de', backgroundColor: '#1b1716' },
  '.cm-activeLine': { backgroundColor: '#ff7a1815' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: '#ff7a1a' },
  '.cm-selectionBackground, ::selection': { backgroundColor: '#683712 !important' },
  '.cm-search': { backgroundColor: '#171312', color: '#f3ece5', padding: '8px' },
  '.cm-search input': { backgroundColor: '#0d0d0f', color: '#f5eee7', border: '1px solid #40383a', borderRadius: '4px' },
  '.cm-search button': { color: '#e7ddd5', background: '#292222', border: '1px solid #4d403b', borderRadius: '4px' },
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
