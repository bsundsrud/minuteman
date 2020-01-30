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
};

const Gauge = {
    view: (vnode) => {
        let value = vnode.attrs.value > 0 ? vnode.attrs.value : "-";
        return (<span className="gauge">{value}</span>);
    }
};

function makeWorkerRow(stats) {
    let s = stats.latest || {};
    let classes = stats.state === "Disconnected" ? "worker-row disconnected" : "worker-row";
    return (<tr className={classes} key={s.id}>
                <td>{stats.id}</td>
                <td><abbr title={stats.socket}>{stats.hostname}</abbr></td>
                <td>{stats.state}</td>
                <td>{s.elapsed ? formatMillis(s.elapsed) : ""}</td>
                <td><Gauge value={s.min}/></td>
                <td><Gauge value={s.mean.toFixed(1)}/></td>
                <td><Gauge value={s.median}/></td>
                <td><Gauge value={s.p90}/></td>
                <td><Gauge value={s.max}/></td>
                <td><Meter elapsedMs={s.elapsed} count={s.count}/></td>
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
            </tr>)
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
                        <tr>
                            <th colspan="4"></th>
                            <th colspan="5">Response Times (ms)</th>
                            <th colspan="6">Request Counts</th>
                        </tr>
                        <tr className="header-row">
                            <th>ID</th>
                            <th>Host</th>
                            <th>State</th>
                            <th>Elapsed</th>
                            <th>Min</th>
                            <th>Mean</th>
                            <th>Median</th>
                            <th>p90</th>
                            <th>Max</th>
                            <th>Requests</th>
                            <th>1xx</th>
                            <th className="bg-green">2xx</th>
                            <th>3xx</th>
                            <th className="bg-yellow">4xx</th>
                            <th className="bg-red">5xx</th>
                        </tr>
                    </thead>
                    <tbody>
                        { StatsApi.data.map(makeWorkerRow) }
                    </tbody>
                </table>);
    }
};

export default WorkerTable;
