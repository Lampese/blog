(() => {
  'use strict';
  const catalog = JSON.parse(document.getElementById('site-text').textContent);
  const pluralRules = new Map();
  function selectPlural(key, count) {
    const language = catalog.pluralLanguages[key] || catalog.language;
    if (!pluralRules.has(language)) {
      try { pluralRules.set(language, new Intl.PluralRules(language)); }
      catch { pluralRules.set(language, new Intl.PluralRules('en')); }
    }
    return pluralRules.get(language).select(Number(count));
  }
  function t(key, arguments_ = {}) {
    let message = catalog.messages[key];
    if (message === undefined) throw Error(`Unknown translation key: ${key}`);
    if (typeof message !== 'string') {
      if (!Object.hasOwn(arguments_, 'count')) throw Error(`Missing count for translation: ${key}`);
      message = message[selectPlural(key, arguments_.count)] ?? message.other;
    }
    return message.replace(/\{([A-Za-z0-9_]+)\}/g, (_, name) => {
      if (!Object.hasOwn(arguments_, name)) throw Error(`Missing argument ${name} for translation: ${key}`);
      return String(arguments_[name]);
    });
  }
  window.blogI18n = Object.freeze({language:catalog.language, t});
})();
