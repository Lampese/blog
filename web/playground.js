import {EditorView, keymap, drawSelection, highlightSpecialChars} from '@codemirror/view';
import {EditorState} from '@codemirror/state';
import {defaultKeymap, history, historyKeymap} from '@codemirror/commands';
import {HighlightStyle, syntaxHighlighting, bracketMatching, indentOnInput} from '@codemirror/language';
import {javascript} from '@codemirror/lang-javascript';
import {tags} from '@lezer/highlight';

const {t} = window.blogI18n;

const colors = HighlightStyle.define([
  {tag: tags.keyword, color: '#9a3659'},
  {tag: [tags.string, tags.regexp], color: '#356844'},
  {tag: [tags.number, tags.bool, tags.null], color: '#945c18'},
  {tag: [tags.function(tags.variableName), tags.function(tags.propertyName)], color: '#235f88'},
  {tag: [tags.typeName, tags.className], color: '#805821'},
  {tag: tags.comment, color: '#697365', fontStyle: 'italic'},
  {tag: tags.operator, color: '#805066'},
  {tag: tags.punctuation, color: '#62685d'},
]);
const runButton = document.querySelector('#run');
const stopButton = document.querySelector('#stop');
const output = document.querySelector('#output');
const status = document.querySelector('#status');
let editor, worker, timer, initialized = false;

function workerMain() {
  const format = value => {
    if (typeof value === 'string') return value;
    if (value instanceof Error) return `${value.name}: ${value.message}`;
    if (value === undefined) return 'undefined';
    try { return JSON.stringify(value) ?? String(value); } catch { return String(value); }
  };
  self.onmessage = async event => {
    const log = (...values) => self.postMessage({type:'log', text:values.map(format).join(' ')});
    try {
      const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
      const result = await new AsyncFunction('console', event.data.code)({log, warn:log, error:log, info:log});
      if (result !== undefined) log(result);
      self.postMessage({type:'done'});
    } catch (error) { self.postMessage({type:'error', text:format(error)}); }
  };
}
function stop(message) {
  clearTimeout(timer); worker?.terminate(); worker = null;
  stopButton.hidden = true; runButton.disabled = false;
  if (message) status.textContent = message;
}
function run() {
  if (!editor) return true;
  stop(); output.textContent = ''; status.textContent = t('running');
  try {
    const blob = new Blob([`(${workerMain.toString()})()`], {type:'text/javascript'});
    const url = URL.createObjectURL(blob);
    worker = new Worker(url); URL.revokeObjectURL(url);
    runButton.disabled = true; stopButton.hidden = false;
    worker.onmessage = event => {
      const value = event.data;
      if (value.type === 'log') {
        if (output.textContent.length < 40000) output.textContent += value.text + '\n';
      } else if (value.type === 'error') { output.textContent += value.text + '\n'; stop(t('execution_error')); }
      else if (value.type === 'done') stop(t('execution_complete'));
    };
    worker.onerror = event => { output.textContent = event.message || t('execution_unavailable'); stop(t('execution_error')); };
    worker.postMessage({code:editor.state.doc.toString()});
    timer = setTimeout(() => stop(t('execution_timeout', {seconds:3})), 3000);
  } catch (error) { output.textContent = error.message; stop(t('execution_error')); }
  return true;
}
runButton.addEventListener('click', run);
stopButton.addEventListener('click', () => stop(t('execution_stopped')));
window.addEventListener('message', event => {
  if (event.source !== parent || event.data?.type !== 'blog-playground-init' || initialized || typeof event.data.code !== 'string') return;
  initialized = true;
  editor = new EditorView({
    state: EditorState.create({doc:event.data.code, extensions:[
      javascript(), syntaxHighlighting(colors), history(), drawSelection(), highlightSpecialChars(),
      bracketMatching(), indentOnInput(), EditorView.lineWrapping,
      EditorView.contentAttributes.of({'aria-label':t('editor_label'), spellcheck:'false'}),
      keymap.of([{key:'Mod-Enter', run}, ...defaultKeymap, ...historyKeymap]),
      EditorView.theme({
        '&':{height:'164px', background:'transparent', color:'#252620', fontSize:'14px'},
        '&.cm-focused':{outline:'none'},
        '.cm-scroller':{fontFamily:'Plex, ui-monospace, monospace', lineHeight:'1.8', overflow:'auto'},
        '.cm-content':{padding:'14px 16px', caretColor:'#252620'},
        '.cm-line':{padding:'0'},
        '.cm-cursor':{borderLeftColor:'#252620'},
        '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection':{background:'#dfd0b4'},
      }),
    ]}),
    parent:document.querySelector('#editor'),
  });
  runButton.disabled = false; status.textContent = '';
});
parent.postMessage({type:'blog-playground-ready'}, '*');
