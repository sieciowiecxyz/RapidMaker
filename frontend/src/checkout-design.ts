import { mount } from 'svelte';
import '@fontsource-variable/manrope';
import '@fontsource-variable/oxanium';
import './storefront/app.css';
import './storefront/light.css';
import Page from './storefront/components/OrderPage.svelte';
import { getMaterials } from './storefront/lib/api';
const materials=await getMaterials();
mount(Page,{target:document.getElementById('app')!,props:{draft:{materials,models:[{id:'design-sample',name:'uchwyt-montazowy.stl',quantity:2,sample:true,settings:{material:'pla',color:'black',customShade:'',quality:'standard',strength:'reinforced'}}],quote:{lines:[],total:0,delivery:0,manual:true},delivery:'shipping'},onback:()=>location.assign('/#wycena')}});
