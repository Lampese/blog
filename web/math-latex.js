import {MathMLToLaTeX} from 'mathml-to-latex';

const supported = new Set('math mi mo mn msqrt mfenced mfrac mroot menclose mphantom msup msub msubsup mmultiscripts mtext munderover mtable mtr mtd mover munder mrow mspace mpadded mstyle none mprescripts semantics annotation'.split(' '));
const textCommands = {'\\':'\\textbackslash{}','{':'\\{','}':'\\}','$':'\\$','&':'\\&','#':'\\#','%':'\\%','_':'\\_','^':'\\textasciicircum{}','~':'\\textasciitilde{}'};
const accents = new Map([
  ['\u0300','grave'], ['\u0301','acute'], ['\u0302','hat'], ['\u0303','tilde'],
  ['\u0304','bar'], ['\u0305','overline'], ['\u0306','breve'], ['\u0307','dot'],
  ['\u0308','ddot'], ['\u030c','check'], ['\u20d7','vec'], ['\u23de','overbrace'],
]);

/** Convert an isolated clone; the published MathML and CRDT atom stay intact. */
export function toLatex(math) {
  let clone = math.cloneNode(true);
  if (clone.localName !== 'math') {
    const wrapper = clone.ownerDocument.createElement('math');
    wrapper.appendChild(clone);
    clone = wrapper;
  }
  const annotations = [...clone.getElementsByTagName('annotation')];
  const tex = annotations.find(node => ['application/x-tex','application/x-latex'].includes(node.getAttribute('encoding')));
  if (tex?.textContent.trim()) return tex.textContent.trim();
  for (const node of annotations) node.parentNode.removeChild(node);
  const nodes = [clone, ...clone.getElementsByTagName('*')];
  const replacements = new Map();
  let prefix = 'BLOGPROTECTEDTEXT';
  while (clone.textContent.includes(prefix)) prefix += 'X';
  function protect(node, latex) {
    const marker = `${prefix}${replacements.size}`;
    replacements.set(marker, latex);
    const token = node.ownerDocument.createElement('mi');
    token.textContent = marker;
    node.parentNode.replaceChild(token, node);
  }
  for (const node of nodes) {
    if (!supported.has(node.localName)) throw Error(`Unsupported MathML element: ${node.localName}`);
    const children = [...node.childNodes].filter(child => child.nodeType === 1);
    if (node.localName === 'mtable' && node.getAttribute('class')?.split(/\s+/).includes('multiline-equation')) {
      const aligned = node.getAttribute('class').split(/\s+/).includes('aligned');
      const environment = aligned ? 'aligned' : 'gathered';
      const rows = children.map(row => [...row.childNodes].filter(child => child.nodeType === 1).map(toLatex).join(aligned ? ' & ' : ' '));
      protect(node, `\\begin{${environment}} ${rows.join(' \\\\ ')} \\end{${environment}}`);
      continue;
    }
    if (node.localName === 'mover' && node.getAttribute('accent') === 'true' && children.length === 2) {
      const command = accents.get(children[1].textContent);
      if (command) {
        protect(node, `\\${command}{${toLatex(children[0])}}`);
        continue;
      }
    }
    if (node.localName !== 'mtext') continue;
    const marker = `${prefix}${replacements.size}`;
    const escaped = node.textContent.replace(/[\\{}$&#%_^~]/g, char => textCommands[char]);
    replacements.set(marker, escaped);
    node.textContent = marker;
  }
  const latex = MathMLToLaTeX.convert(clone.outerHTML || clone.toString());
  if (!latex) throw Error('Empty LaTeX conversion');
  // Restore text once so literal text cannot be interpreted as converter syntax.
  return latex.replace(new RegExp(prefix + '\\d+', 'g'), marker => replacements.get(marker) ?? marker);
}
