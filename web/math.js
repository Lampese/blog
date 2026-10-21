const {t} = window.blogI18n;
const converterURL = document.currentScript?.dataset.converter;
let converter;
const formulas = [...document.querySelectorAll('.reading-content math')].filter(math => !math.closest('a'));
if (formulas.length && typeof HTMLElement.prototype.showPopover === 'function') {
  const panel = document.createElement('div');
  panel.id = 'math-tools';
  panel.setAttribute('popover', 'auto');
  panel.setAttribute('role', 'dialog');
  panel.setAttribute('aria-label', t('latex_source'));
  const label = document.createElement('label');
  label.htmlFor = 'math-latex';
  label.textContent = t('latex_source');
  const source = document.createElement('textarea');
  source.id = 'math-latex';
  source.readOnly = true;
  source.spellcheck = false;
  source.wrap = 'off';
  const footer = document.createElement('div');
  const copy = document.createElement('button');
  copy.type = 'button';
  copy.textContent = t('copy_latex');
  const status = document.createElement('span');
  status.setAttribute('role', 'status');
  footer.append(copy, status);
  panel.append(label, source, footer);
  document.body.append(panel);

  let active;
  const cache = new WeakMap();
  function position() {
    if (!active || !panel.matches(':popover-open')) return;
    const anchor = active.getBoundingClientRect();
    if (anchor.bottom < 0 || anchor.top > innerHeight) {panel.hidePopover(); return;}
    const bounds = panel.getBoundingClientRect(), gap = 10, edge = 12;
    const left = Math.min(Math.max(edge, anchor.left), innerWidth - bounds.width - edge);
    const below = anchor.bottom + gap;
    const top = below + bounds.height <= innerHeight - edge ? below : Math.max(edge, anchor.top - bounds.height - gap);
    panel.style.left = `${left}px`;
    panel.style.top = `${top}px`;
  }
  let request = 0;
  async function open(math) {
    const current = ++request;
    active?.setAttribute('aria-expanded', 'false');
    active = math;
    math.scrollIntoView({block:'nearest', inline:'nearest', behavior:'instant'});
    status.textContent = t('latex_loading');
    source.value = '';
    source.rows = 2;
    copy.disabled = true;
    panel.showPopover();
    math.setAttribute('aria-expanded', 'true');
    position();
    try {
      converter ||= import(converterURL).catch(error => {converter = null; throw error;});
      const {toLatex} = await converter;
      if (current !== request || !panel.matches(':popover-open')) return;
      if (!cache.has(math)) cache.set(math, toLatex(math));
      source.value = cache.get(math);
      if (!source.value) throw Error('Empty LaTeX conversion');
      copy.disabled = false;
      status.textContent = '';
    } catch {
      if (current !== request || !panel.matches(':popover-open')) return;
      source.value = '';
      copy.disabled = true;
      status.textContent = t('latex_unavailable');
    }
    source.rows = Math.min(6, Math.max(2, source.value.split('\n').length));
    position();
    (copy.disabled ? source : copy).focus({preventScroll:true});
  }
  for (const math of formulas) {
    let pointer;
    math.tabIndex = 0;
    math.setAttribute('aria-haspopup', 'dialog');
    math.setAttribute('aria-controls', panel.id);
    math.setAttribute('aria-expanded', 'false');
    math.setAttribute('title', t('view_latex'));
    math.addEventListener('pointerdown', event => {pointer = [event.clientX, event.clientY];});
    math.addEventListener('click', event => {
      const dragged = pointer && Math.hypot(event.clientX - pointer[0], event.clientY - pointer[1]) > 4;
      pointer = null;
      if (!dragged && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey) open(math);
    });
    math.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {event.preventDefault(); open(math);}
    });
  }
  panel.addEventListener('toggle', () => {
    if (!panel.matches(':popover-open')) active?.setAttribute('aria-expanded', 'false');
  });
  window.addEventListener('scroll', position, {capture:true, passive:true});
  window.addEventListener('resize', position, {passive:true});
  copy.addEventListener('click', async () => {
    try {
      // Copy the visible source during the user gesture, including on HTTP and
      // in browsers whose asynchronous clipboard API needs extra permission.
      source.focus({preventScroll:true}); source.select();
      let supplied = false;
      const supply = event => {
        if (!event.clipboardData) return;
        event.clipboardData.setData('text/plain', source.value);
        event.preventDefault();
        supplied = true;
      };
      document.addEventListener('copy', supply, {once:true});
      let copied = false;
      try { copied = document.execCommand('copy'); }
      finally { document.removeEventListener('copy', supply); }
      if (!copied || !supplied) {
        if (!navigator.clipboard?.writeText) throw Error('Clipboard unavailable');
        await navigator.clipboard.writeText(source.value);
      }
      copy.focus({preventScroll:true});
      status.textContent = t('latex_copied');
    } catch {
      source.focus({preventScroll:true}); source.select();
      status.textContent = t('latex_copy_failed');
    }
  });
}
