import { mount } from 'svelte';
import '@fontsource-variable/manrope';
import '@fontsource-variable/oxanium';
import './theme.css';
import './admin.css';
import Admin from './Admin.svelte';
mount(Admin, { target: document.getElementById('app')! });
