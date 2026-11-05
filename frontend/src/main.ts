import { mount } from 'svelte';
import App from './App.svelte';
import '@fontsource-variable/manrope';
import '@fontsource-variable/oxanium';
import { restoreTheme } from './lib/theme';
restoreTheme();
mount(App, { target: document.getElementById('app')! });
