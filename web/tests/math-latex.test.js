import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {DOMParser} from '@xmldom/xmldom';
import {toLatex} from '../math-latex.js';

const parse = markup => new DOMParser().parseFromString(markup, 'application/xml').documentElement;
const fixtures = JSON.parse(await readFile(new URL('fixtures/formulas.json', import.meta.url), 'utf8'));

test('Typst MathML preserves fractions, limits, matrices, cases, alignment, roots, accents and text', () => {
  for (const {mathml, latex} of fixtures) {
    const math = parse(mathml), original = math.toString();
    assert.equal(toLatex(math), latex);
    assert.equal(math.toString(), original, 'Conversion must not mutate reference atoms');
  }
});

test('nested accents and text restore without leaving internal placeholders', () => {
  const math = parse('<math><mover accent="true"><mrow><mtext>a_b</mtext><mover accent="true"><mi>x</mi><mo>&#x0302;</mo></mover></mrow><mo>&#x20d7;</mo></mover></math>');
  assert.equal(toLatex(math), String.raw`\vec{\text{a\_b} \hat{x}}`);
});

test('an explicit TeX annotation takes precedence and unsupported elements fail clearly', () => {
  const math = parse('<math><semantics><mi>x</mi><annotation encoding="application/x-tex">\\custom{x}</annotation></semantics></math>');
  assert.equal(toLatex(math), String.raw`\custom{x}`);
  assert.throws(() => toLatex(parse('<math><unknown><mi>x</mi></unknown></math>')), /Unsupported MathML element/);
  assert.throws(() => toLatex(parse('<math/>')), /Empty LaTeX/);
});
