import m from 'mithril';
import { ControlsApi } from "../api";
import './Controls.css';

const Controls = {
    view: () => {
        return (<div className="controls">
                    <div className="url">
                        <span className="label">Url</span>
                        <input type="text"
                               oninput={(e) => ControlsApi.setUrl(e.target.value)}
                               value={ControlsApi.url}/>
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
                                <a className="btn launch"
                                   onclick={(e) => ControlsApi.start()}>
                                    Start
                                </a>
                            </li>
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
                            <li>
                                <a className="btn clear"
                                   onclick={(e) => ControlsApi.clear_disconnected()}>
                                    Clear
                                </a>
                            </li>
                        </ul>
                    </div>
                </div>);
    }
};

export default Controls;
