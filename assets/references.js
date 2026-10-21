(() => {
  'use strict';
  const {t} = window.blogI18n;
  const prefix = document.currentScript?.dataset.prefix || '';
  const reader = document.documentElement.classList.contains('reference-document');
  const origin = location.origin;
  const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
  function articleURL(value) {
    let url; try { url = new URL(value, location.href); } catch { return null; }
    const path = url.pathname.slice((prefix + '/posts/').length);
    if (url.origin !== origin || !url.pathname.startsWith(prefix + '/posts/') || !/^[a-z0-9-]+\/$/.test(path)) return null;
    url.searchParams.delete('reader');
    if (url.hash) url.searchParams.delete('range');
    return url;
  }
  function linkedArticle(link) {
    const url = articleURL(link.href);
    if (!url) return null;
    const local = url.pathname === location.pathname;
    return reader || !local || url.hash || url.searchParams.has('range') || link.hasAttribute('data-blog-reference') ? url : null;
  }
  // A hash inside a reader should address the article, not inherit the reader's
  // previous range query. Modified clicks still open the ordinary full page.
  if (reader) for (const link of document.querySelectorAll('a[href]')) {
    if (link.closest('.article-sidebar')) continue;
    const url = articleURL(link.href);
    if (url) link.href = url.href;
    else if (!link.hasAttribute('download')) { link.target = '_blank'; link.rel = 'noopener'; }
  }
  const current = new URL(location.href);
  if (current.hash) {
    let id; try {id = decodeURIComponent(current.hash.slice(1));} catch {id = current.hash.slice(1);}
    const target = document.getElementById(id);
    if (target) {
      const section = [...document.querySelectorAll('.article-sidebar a[data-section]')].some(link=>link.dataset.section===id);
      if (!section) target.classList.add('reference-target');
      requestAnimationFrame(() => target.scrollIntoView({block:section?'start':'center', behavior:'instant'}));
    }
  }

  const workspace = document.getElementById('reading-workspace');
  const shell = document.getElementById('site-shell');
  const deck = document.getElementById('reference-panels');
  const controls = document.getElementById('reference-controls');
  const counter = document.getElementById('reference-count');
  if (!reader && (!workspace || !shell || !deck || !controls)) return;
  const panels = [];
  const storageKey = 'blog-reading-workspace-v1';
  let nextId = 0, firstOpener, saveScheduled = false;
  function save() {
    if (reader || !panels.length) return;
    try {sessionStorage.setItem(storageKey, JSON.stringify({url:location.href, links:panels.map(p=>p.url.href), top:shell.scrollTop, left:workspace.scrollLeft}));} catch {}
  }
  function scheduleSave() {
    if (saveScheduled) return;
    saveScheduled = true; requestAnimationFrame(() => {saveScheduled = false;save();});
  }
  function updateCount() {counter.textContent = t('references_open', {count:panels.length});}
  function closeAll() {
    if (!panels.length) return;
    const top = shell.scrollTop;
    for (const panel of panels) panel.node.remove();
    panels.length = 0; deck.hidden = true; controls.hidden = true;
    document.body.classList.remove('has-reference-panels'); workspace.scrollLeft = 0; shell.scrollTop = 0;
    window.scrollTo({top, left:0, behavior:'instant'});
    try {sessionStorage.removeItem(storageKey);} catch {}
    if (firstOpener?.isConnected) firstOpener.focus({preventScroll:true});
  }
  function closePanel(panel) {
    if (panels.length === 1) return closeAll();
    const index = panels.indexOf(panel); if (index < 0) return;
    panel.node.remove();panels.splice(index,1);updateCount();save();
    const previous = panels[Math.max(0,index-1)];
    if (panel.opener?.isConnected) panel.opener.focus({preventScroll:true});
    else previous?.close.focus({preventScroll:true});
  }
  function reveal(panel, focus = true) {
    panel.node.scrollIntoView({inline:'end',block:'nearest',behavior:reducedMotion()?'instant':'smooth'});
    if (focus) panel.node.focus({preventScroll:true});
  }
  function openReference(value, label, opener, restoring = false) {
    const url = articleURL(value); if (!url || reader) return;
    const existing = panels.find(p=>p.url.href===url.href);
    if (existing) {reveal(existing);return;}
    if (!panels.length) {
      const top = window.scrollY; firstOpener = opener;
      document.body.classList.add('has-reference-panels'); deck.hidden = false; controls.hidden = false;
      window.scrollTo({top:0,left:0,behavior:'instant'});shell.scrollTop = top;
    }
    const node = document.createElement('section');node.className='reference-panel';node.tabIndex=-1;node.setAttribute('role','dialog');
    const id = `reference-title-${++nextId}`;node.setAttribute('aria-labelledby',id);
    const bar=document.createElement('header');bar.className='reference-panel-header';
    const heading=document.createElement('h2');heading.id=id;heading.textContent=label || t('source_article');
    const standalone=document.createElement('a');standalone.href=url.href;standalone.target='_blank';standalone.rel='noopener';standalone.textContent=t('open_separately');
    const close=document.createElement('button');close.type='button';close.textContent='×';close.setAttribute('aria-label',t('close_reference'));
    bar.append(heading,standalone,close);
    const frame=document.createElement('iframe');const embedded=new URL(url.href);embedded.searchParams.set('reader','1');frame.src=embedded.href;frame.title=t('source_title', {title:heading.textContent});
    const status=document.createElement('p');status.className='reference-panel-status';status.setAttribute('role','status');status.textContent=t('loading_source');
    node.append(bar,status,frame);deck.append(node);
    const panel={url,node,frame,heading,status,close,opener};panels.push(panel);
    close.addEventListener('click',()=>closePanel(panel));
    frame.addEventListener('load',()=>{if(!panel.ready){status.textContent='';status.hidden=true;}});
    updateCount();if(!restoring)reveal(panel);save();
  }
  document.addEventListener('click', event => {
    if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const link=event.target.closest?.(reader?'a[href]':'#article-content a[href]');
    if (!link || link.hasAttribute('download') || (link.target && link.target!=='_self')) return;
    if (link.closest('.article-sidebar')) return;
    const url=linkedArticle(link);
    if(!url){if(reader){link.target='_blank';link.rel='noopener';}return;}
    event.preventDefault();
    if(reader) {document.querySelector('dialog[open]')?.close();parent.postMessage({type:'blog-reference-open',url:url.href,label:link.textContent.trim()},origin);}
    else openReference(url.href,link.textContent.trim(),link);
  });
  document.addEventListener('keydown', event => {
    if(event.key!=='Escape' || event.defaultPrevented || document.querySelector('dialog[open]'))return;
    if([...document.querySelectorAll('[popover]')].some(node=>typeof node.showPopover==='function' && node.matches(':popover-open')))return;
    if(reader) parent.postMessage({type:'blog-reference-close'},origin);
    else if(panels.length) closePanel(panels.at(-1));
  });
  if(reader){
    requestAnimationFrame(()=>{
      const meta=JSON.parse(document.getElementById('post-data')?.textContent || 'null');
      const range=current.searchParams.get('range');
      parent.postMessage({type:'blog-reference-ready',title:document.querySelector('.article-heading h1')?.textContent,
        invalid:!!range && !meta?.references?.[range]},origin);
    });
    return;
  }
  window.addEventListener('message', event => {
    if(event.origin!==origin)return;
    const panel=panels.find(p=>p.frame.contentWindow===event.source);if(!panel)return;
    const message=event.data;
    if(message?.type==='blog-reference-open')openReference(message.url,message.label,panel.frame);
    else if(message?.type==='blog-reference-close')closePanel(panel);
    else if(message?.type==='blog-reference-ready'){
      panel.ready=true;
      if(typeof message.title==='string'){panel.heading.textContent=message.title;panel.frame.title=t('source_title', {title:message.title});}
      panel.status.textContent=message.invalid?t('invalid_reference'):'';panel.status.hidden=!message.invalid;
    }
  });
  document.getElementById('references-close-all')?.addEventListener('click',closeAll);
  workspace?.addEventListener('scroll',scheduleSave,{passive:true});shell?.addEventListener('scroll',scheduleSave,{passive:true});
  window.addEventListener('pagehide',save);
  try {
    const saved=JSON.parse(sessionStorage.getItem(storageKey)||'null');
    if(saved?.url===location.href && Array.isArray(saved.links)){
      for(const url of saved.links)openReference(url,t('source_article'),null,true);
      if(panels.length)requestAnimationFrame(()=>{shell.scrollTop=Number(saved.top)||0;workspace.scrollLeft=Number(saved.left)||0;save();});
    }
  } catch {}
})();
