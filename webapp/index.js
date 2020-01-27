import m from 'mithril';
import WorkerTable from './components/WorkerTable.jsx';
import Controls from './components/Controls.jsx';
import './app.css';

const controlsEl = document.getElementById("controls");
const dataEl = document.getElementById("data");

m.mount(dataEl, WorkerTable);
m.mount(controlsEl, Controls);
