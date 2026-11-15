// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render } from '@testing-library/react';
import { EditorView } from '@codemirror/view';
import { openSearchPanel } from '@codemirror/search';
import { Editor } from './Editor';

afterEach(cleanup);

describe('Plain CodeMirror editor', () => {
  function renderEditor() {
    const onChange = vi.fn();
    const onSave = vi.fn();
    const onRun = vi.fn();
    const onCursor = vi.fn();
    const onView = vi.fn();
    const result = render(<Editor path="src/main.plain" value={'LET total = 1\nSAY total'} onChange={onChange} onSave={onSave} onRun={onRun} onCursor={onCursor} onView={onView} />);
    const view = onView.mock.calls.map(([candidate]) => candidate).find((candidate) => candidate instanceof EditorView) as EditorView;
    return { ...result, view, onChange, onSave, onRun, onCursor, onView };
  }

  it('renders an editable Plain source view with line numbers', () => {
    const { container, view } = renderEditor();
    expect(container.querySelector('.cm-content[contenteditable="true"]')).not.toBeNull();
    expect(container.querySelectorAll('.cm-lineNumbers .cm-gutterElement').length).toBeGreaterThanOrEqual(2);
    expect(view.state.doc.toString()).toContain('LET total = 1');
  });

  it('reports document edits and cursor position changes', () => {
    const { view, onChange, onCursor } = renderEditor();
    act(() => view.dispatch({ changes: { from: view.state.doc.length, insert: '\nSAY "done"' } }));
    expect(onChange).toHaveBeenLastCalledWith('LET total = 1\nSAY total\nSAY "done"');
    expect(onCursor).toHaveBeenCalled();
  });

  it('binds save and run keyboard shortcuts to the app actions', () => {
    const { view, onSave, onRun } = renderEditor();
    act(() => view.contentDOM.dispatchEvent(new KeyboardEvent('keydown', { key: 's', ctrlKey: true, bubbles: true, cancelable: true })));
    act(() => view.contentDOM.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, bubbles: true, cancelable: true })));
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onRun).toHaveBeenCalledTimes(1);
  });

  it('opens CodeMirror search controls for in-file find and replace', () => {
    const { container, view } = renderEditor();
    act(() => openSearchPanel(view));
    const searchPanel = container.querySelector('.cm-search');
    expect(searchPanel).not.toBeNull();
    expect(searchPanel?.querySelector('input')).not.toBeNull();
  });
});
