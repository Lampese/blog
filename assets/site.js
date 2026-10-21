(() => {
  'use strict';
  const {t} = window.blogI18n;
  const prefix = document.currentScript?.dataset.prefix || '';
  const development = document.currentScript?.dataset.development === 'true';
  const reader = document.documentElement.classList.contains('reference-document');
  const $ = id => document.getElementById(id);
  const contents = document.querySelector('.article-sidebar');
  if (contents) {
    const details = contents.querySelector('details'), media = matchMedia('(min-width: 1200px)');
    let previousMode;
    function layoutContents() {
      const expanded = media.matches && !reader && !document.body.classList.contains('has-reference-panels');
      if (expanded !== previousMode) details.open = expanded;
      previousMode = expanded;
    }
    layoutContents();
    media.addEventListener('change', layoutContents);
    new MutationObserver(layoutContents).observe(document.body, {attributes:true,attributeFilter:['class']});
    const links = [...contents.querySelectorAll('a[data-section]')];
    const headings = links.map(link => document.getElementById(link.dataset.section));
    let scheduled = false;
    function updateContents() {
      scheduled = false;
      let current = 0;
      headings.forEach((heading,index) => {if (heading.getBoundingClientRect().top <= 100) current = index;});
      links.forEach((link,index) => {if (index === current) link.setAttribute('aria-current','location');else link.removeAttribute('aria-current');});
    }
    document.addEventListener('scroll', () => {if (!scheduled) {scheduled=true;requestAnimationFrame(updateContents);}}, {capture:true,passive:true});
    window.addEventListener('resize', updateContents, {passive:true});
    updateContents();
  }
  // Warm likely destinations without taking navigation or browser history over.
  if (!development && !navigator.connection?.saveData && !/^(slow-)?2g$/.test(navigator.connection?.effectiveType || '')) {
    const prefetched = new Set();
    const warmLink = event => {
      const link = event.target.closest?.('a[href]');
      if (!link || link.hasAttribute('download') || (link.target && link.target !== '_self') || prefetched.size >= 6) return;
      let url; try { url = new URL(link.href, location.href); } catch { return; }
      if (url.origin !== location.origin || url.pathname === location.pathname || !(/\/$|\.html$/.test(url.pathname))) return;
      url.hash = '';
      if (prefetched.has(url.href)) return;
      prefetched.add(url.href);
      const hint = document.createElement('link'); hint.rel = 'prefetch'; hint.as = 'document'; hint.href = url.href; document.head.append(hint);
    };
    document.addEventListener('pointerover', warmLink, {passive:true});
    document.addEventListener('focusin', warmLink);
  }
  const playgrounds = () => [...document.querySelectorAll('iframe.playground[data-code]')];
  const initializePlayground = frame => frame.contentWindow?.postMessage({type:'blog-playground-init',code:frame.dataset.code}, '*');
  window.addEventListener('message', event => {
    if (event.data?.type !== 'blog-playground-ready') return;
    const frame = playgrounds().find(frame => frame.contentWindow === event.source);
    if (frame) initializePlayground(frame);
  });
  for (const frame of playgrounds()) {frame.addEventListener('load', () => initializePlayground(frame));initializePlayground(frame);}
  let noticeTimer;
  function notify(message, error = false) { const el = $('notice'); el.textContent = message; el.classList.toggle('error', error); el.hidden = false; clearTimeout(noticeTimer); noticeTimer = setTimeout(() => el.hidden = true, error ? 12000 : 4500); }
  const dialog = $('search-dialog'), input = $('search-input');
  let searchIndex;
  $('search-open')?.addEventListener('click', () => { dialog.showModal(); input.focus(); });
  dialog?.addEventListener('click', event => {
    if (event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom) dialog.close();
  });
  document.addEventListener('keydown', e => { if ((e.key === '/' || ((e.ctrlKey || e.metaKey) && e.key === 'k')) && !/INPUT|TEXTAREA/.test(document.activeElement?.tagName)) { e.preventDefault(); dialog.showModal(); input.focus(); } });
  input?.addEventListener('input', async () => {
    try {
      searchIndex ||= await fetch(prefix + '/search.json').then(r => { if (!r.ok) throw Error(t('search_unavailable')); return r.json(); });
      const query = input.value.trim().toLocaleLowerCase(window.blogI18n.language); const results = $('search-results'); results.replaceChildren();
      if (!query) { $('search-status').textContent = ''; return; }
      const terms = query.split(/\s+/); const matches = searchIndex.filter(p => terms.every(t => [p.title,p.summary,...p.tags,p.text].join(' ').toLocaleLowerCase(window.blogI18n.language).includes(t))).slice(0,20);
      $('search-status').textContent = matches.length ? t('search_results', {count:matches.length}) : t('no_search_results');
      for (const post of matches) { const li = document.createElement('li'), a = document.createElement('a'), p = document.createElement('p'); a.href = post.url; a.textContent = post.title; p.textContent = post.summary; li.append(a,p); results.append(li); }
    } catch (error) { $('search-status').textContent = error.message; }
  });
  let data;
  try { data = JSON.parse($('post-data')?.textContent || 'null'); } catch { return; }
  const article = $('article-content'), tools = $('selection-tools');
  function flatten(node) {
    if (node.nodeType === Node.TEXT_NODE) return node.data;
    if (node.nodeType === Node.ELEMENT_NODE) { if (/^(SCRIPT|STYLE|TEMPLATE)$/.test(node.tagName)) return ''; if (node.hasAttribute('data-blog-token')) return node.getAttribute('data-blog-token'); }
    return Array.from(node.childNodes).map(flatten).join('');
  }
  function pointOffset(node, offset, end) {
    const element = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
    const atom = element.closest?.('[data-blog-token]');
    if (atom && article.contains(atom)) return Number(atom.dataset.blogStart) + (end ? atom.dataset.blogToken.length : 0);
    const range = document.createRange(); range.selectNodeContents(article); range.setEnd(node, offset); return flatten(range.cloneContents()).length;
  }
  let pending, copying = false;
  if (data && article) {
    document.addEventListener('selectionchange', () => {
      if (copying) return;
      const selection = getSelection();
      if (!selection?.rangeCount || selection.isCollapsed) { tools.hidden = true; pending = null; return; }
      const range = selection.getRangeAt(0);
      if (!article.contains(range.startContainer) || !article.contains(range.endContainer)) { tools.hidden = true; pending = null; return; }
      const start = pointOffset(range.startContainer, range.startOffset, false), end = pointOffset(range.endContainer, range.endOffset, true);
      if (start >= end || !flatten(range.cloneContents()).trim()) { tools.hidden = true; return; }
      pending = {start,end}; tools.hidden = false;
    });
    tools.addEventListener('mousedown', e => e.preventDefault());
    $('dismiss-selection').addEventListener('click', () => { tools.hidden = true; pending = null; getSelection()?.removeAllRanges(); });
    let seed = data.seed;
    $('copy-reference').addEventListener('click', async () => {
      if (!pending || copying) return; copying = true; const selection = {...pending}; const button = $('copy-reference'); button.textContent = t('generating_reference');
      try {
        let code;
        if (development) {
          const response = await fetch(prefix + '/_blog/references', {method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({article:data.article,revision:data.revision,...selection})});
          const result = await response.json(); if (!response.ok) throw Error(result.error || t('reference_failed')); code = result.code;
        } else {
          seed ||= await fetch(data.snapshot).then(async r => { if (!r.ok) throw Error(t('snapshot_unavailable')); const bytes = new Uint8Array(await r.arrayBuffer()); let binary = ''; for (let i=0;i<bytes.length;i+=8192) binary += String.fromCharCode(...bytes.subarray(i,i+8192)); return btoa(binary); });
          const id = 'r-web-' + (crypto.randomUUID?.() || Array.from(crypto.getRandomValues(new Uint8Array(16)),b=>b.toString(16).padStart(2,'0')).join(''));
          code = `#excerpt((\n  schema: 1,\n  article: ${JSON.stringify(data.article)},\n  range: ${JSON.stringify(id)},\n  receipt: (start: ${selection.start}, end: ${selection.end}, seed: "${seed}"),\n))`;
        }
        if (navigator.clipboard?.writeText) await navigator.clipboard.writeText(code);
        else { const area = document.createElement('textarea'); area.value=code; area.style.position='fixed'; area.style.opacity='0'; document.body.append(area); area.select(); const ok=document.execCommand('copy'); area.remove(); if(!ok)throw Error(t('clipboard_unavailable')); }
        notify(t('reference_copied')); tools.hidden = true;
      } catch (error) { notify(error.message,true); } finally { copying=false; button.textContent=t('copy_reference'); }
    });
    const id = new URL(location.href).searchParams.get('range');
    if (id && data.references[id]) {
      const [start,end] = data.references[id], range = document.createRange(); let foundStart=false, foundEnd=false;
      for (const node of article.querySelectorAll('[data-blog-start]')) {
        const a=Number(node.dataset.blogStart), token=node.dataset.blogToken, length=token?.length ?? node.textContent.length;
        if (!foundStart && start>=a && start<a+length) { if(token)range.setStartBefore(node);else range.setStart(node.firstChild,start-a); foundStart=true; }
        if (foundStart && end>a && end<=a+length) { if(token)range.setEndAfter(node);else range.setEnd(node.firstChild,end-a); foundEnd=true; break; }
      }
      if(foundStart&&foundEnd){if(window.Highlight&&CSS.highlights)CSS.highlights.set('blog-reference',new Highlight(range));else range.startContainer.parentElement.classList.add('reference-target');requestAnimationFrame(()=>range.startContainer.parentElement.scrollIntoView({block:'center',behavior:'instant'}));}
    }
  }
  if(development&&!reader){const events=new EventSource(prefix+'/_blog/events');events.onmessage=e=>{const event=JSON.parse(e.data);if(event.error)notify(event.error,true);else if(!copying&&!getSelection()?.toString())location.reload();};}
})();
