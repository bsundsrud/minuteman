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
                        <input type="button" className="btn"
                               onclick={(e) => ControlsApi.start()}
                               value="Start"
                        />
                        <input type="button" className="btn"
                               onclick={(e) => ControlsApi.stop()}
                               value="Stop"
                        />
                        <input type="button" className="btn"
                               onclick={(e) => ControlsApi.reset()}
                               value="Reset"
                        />
                    </div>
                </div>);
    }
};

export default Controls;
