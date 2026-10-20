import { mount } from 'svelte';
import '@fontsource-variable/manrope';
import '@fontsource-variable/oxanium';
import './app.css';
import './transitions.css';
import App from './App.svelte';
mount(App, { target: document.getElementById('app')! });
