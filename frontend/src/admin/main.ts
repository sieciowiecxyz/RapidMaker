import { mount } from 'svelte';
import '@fontsource-variable/manrope';
import '@fontsource-variable/oxanium';
import './theme.css';
import './admin.css';
import Admin from './Admin.svelte';
import { restoreTheme } from '../lib/theme';
restoreTheme();
mount(Admin, { target: document.getElementById('app')! });
