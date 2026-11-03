// Report startup errors without treating blocked document images as renderer failures.
window.addEventListener('error', (event) => {
  if (event.target !== window && !(event.target instanceof HTMLScriptElement) && !(event.target instanceof HTMLLinkElement)) return;
  const message = event.message || event.target?.src || event.target?.href || 'Renderer resource failed to load';
  window.webkit?.messageHandlers?.devoraRenderer?.postMessage({ error: message });
}, true);
