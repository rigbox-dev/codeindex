import React from 'react';
import { createRoot } from 'react-dom/client';
import GraphApp from './GraphApp';

// Inherit theme from parent window (when loaded in iframe)
try {
  const parentTheme = window.parent.document.documentElement.getAttribute('data-theme');
  if (parentTheme) document.documentElement.setAttribute('data-theme', parentTheme);
} catch (e) {}

createRoot(document.getElementById('root')!).render(<GraphApp />);
