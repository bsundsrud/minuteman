import m from 'mithril';
import './WorkerTable.css';
import { StatsApi } from '../api';

function formatMillis(ms) {
    var res = "";
    while (true) {
        if (ms > 60000) {
            const mins = Math.floor(ms / 60000);
            res = res + mins + "m";
            ms = ms % 60000;
        } else if (ms > 10000) {
            const secs = Math.floor(ms / 1000);
            res = res + secs + "s";
            break
        } else if (ms > 2000) {
            const secs = Math.floor(ms / 1000);
            res = res + secs + "s";
            ms = ms % 1000;
        } else {
            res = res + ms + "ms";
            break
        }
    }
    return res;
}

const Meter = {

    view: (vnode) => {
        let elapsed = vnode.attrs.elapsedMs / 1000;
        let reqs = vnode.attrs.count;
        let rate = elapsed > 0 ? (reqs / elapsed).toFixed(1) : 0;
        return (<span className="meter">
                    <span className="meter--count">{reqs || "-"}</span>
                    {rate > 0 ? <span className="meter--rate">({rate}/s)</span> : ""}
                </span>);
    }
}

const WorkerTable = {
    oninit: (vnode) => {
        StatsApi.fetch();
        let interval = setInterval(StatsApi.fetch, 5000);
        vnode.state.interval = interval;
    },
    view:() => {
        return (<table className="worker-table">
                    <thead>
                        <tr className="header-row">
                            <th>Host</th>
                            <th>Socket</th>
                            <th>State</th>
                            <th>Elapsed</th>
                            <th>Requests</th>
                            <th>1xx</th>
                            <th className="bg-green">2xx</th>
                            <th>3xx</th>
                            <th className="bg-yellow">4xx</th>
                            <th className="bg-red">5xx</th>
                        </tr>
                    </thead>
                    <tbody>
                        { StatsApi.data.map((s) => {
                            return (<tr className="worker-row" key={s.socket}>
                                        <td>{s.hostname}</td>
                                        <td>{s.socket}</td>
                                        <td>{s.state}</td>
                                        <td>{s.elapsed ? formatMillis(s.elapsed) : ""}</td>
                                        <td>{s.count}</td>
                                        <td><Meter elapsedMs={s.elapsed} count={s.count_1xx}/></td>
                                        <td className="bg-green">
                                            <Meter elapsedMs={s.elapsed} count={s.count_2xx} />
                                        </td>
                                        <td>
                                            <Meter elapsedMs={s.elapsed} count={s.count_3xx} />
                                        </td>
                                        <td className="bg-yellow">
                                            <Meter elapsedMs={s.elapsed} count={s.count_4xx}/>
                                        </td>
                                        <td className="bg-red">
                                            <Meter elapsedMs={s.elapsed} count={s.count_5xx}/>
                                        </td>
                                    </tr>);
                        }) }
                    </tbody>
                </table>);
    }
};

export default WorkerTable;
