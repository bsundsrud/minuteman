import m from 'mithril';
import { ControlsApi } from "../api";
import './Controls.css';
import SpecEditor from './SpecEditor';

const possibleStrategies = [
    {id: "Random", display: "Random"},
    {id: "InOrder", display: "In Order"},
];

function indexOfStrategy(id) {
    for (var i = 0; i < possibleStrategies.length; i++) {
        if (id === possibleStrategies[i].id) {
            return i;
        }
    }
    return -1;
}


function Controls(initial) {
    function launchDisabled() {
        return ControlsApi.specs.length == 0;
    }

    function launchButtonClick(e) {
        if (launchDisabled()) {
            return;
        } else {
            ControlsApi.start();
        }
    }

    function launchButtonCss(e) {
        if (launchDisabled()) {
            return "round-btn launch disabled";
        } else {
            return "round-btn launch";
        }
    }

    return {
        view: () => {
            return (<div className="controls">
                        <SpecEditor specs={ControlsApi.specs} />
                        <div className="controls-global">
                            <h3 className="controls-global-header">Test Settings</h3>
                            <div className="strategy">
                                <span className="label">Strategy</span>
                                <select selectedIndex={indexOfStrategy(ControlsApi.strategy)}
                                        onchange={ (e) => ControlsApi.strategy = e.target.value }>
                                    { possibleStrategies.map((s) =>
                                        <option value={ s.id }> { s.display }</option>
                                    ) }
                                </select>
                            </div>
                            <div className="concurrency">
                                <span className="label">Max Concurrency</span>
                                <input type="number"
                                       required="true"
                                       oninput={(e) => ControlsApi.setConcurrency(e.target.value)}
                                       value={ControlsApi.concurrency}/>
                            </div>
                            <div className="actions">
                                <ul className="action-list">
                                    <li>
                                        <a className="btn stop"
                                           onclick={(e) => ControlsApi.stop()}>
                                            Stop
                                        </a>
                                    </li>
                                    <li>
                                        <a className="btn reset"
                                           onclick={(e) => ControlsApi.reset()}>
                                            Reset
                                        </a>
                                    </li>
                                </ul>
                            </div>
                        </div>
                        <div className="launch-controls">
                            <div className="launch-container">
                                <a className={launchButtonCss()}
                                   onclick={launchButtonClick}>
                                    Start
                                </a>
                            </div>
                        </div>
                    </div>
                   );
        },
    }
};

export default Controls;
