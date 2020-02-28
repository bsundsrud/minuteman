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
        let rate = (vnode.attrs.rate || 0).toFixed(1);
        let reqs = vnode.attrs.count;
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
    let queueStats = `${s.tasks}/${s.task_queue}/${s.tasks_max}`;
    return (<tr className={classes} key={s.id}>
                <td>{stats.id}</td>
                <td><abbr title={stats.socket}>{stats.hostname}</abbr></td>
                <td>{stats.state}</td>
                <td>{s.elapsed ? formatMillis(s.elapsed) : ""}</td>
                <td><Gauge value={s.min}/></td>
                <td><Gauge value={s.mean.toFixed(1)}/></td>
                <td><Gauge value={s.stdev.toFixed(1)}/></td>
                <td><Gauge value={s.median}/></td>
                <td><Gauge value={s.p90}/></td>
                <td><Gauge value={s.max}/></td>
                <td><Meter rate={s.rate_count} count={s.count}/></td>
                <td><Meter rate={s.rate_1xx} count={s.count_1xx}/></td>
                <td className="bg-green">
                    <Meter rate={s.rate_2xx} count={s.count_2xx} />
                </td>
                <td>
                    <Meter rate={s.rate_3xx} count={s.count_3xx} />
                </td>
                <td className="bg-yellow">
                    <Meter rate={s.rate_4xx} count={s.count_4xx}/>
                </td>
                <td className="bg-red">
                    <Meter rate={s.rate_5xx} count={s.count_5xx}/>
                </td>
                <td className="bg-red">
                    <Meter rate={s.rate_fail} count={s.count_fail}/>
                </td>
                <td>{queueStats}</td>
            </tr>)
}

function makeSummaryRow(stats) {
    let initial = {
        elapsed: 0,
        min: undefined,
        mean: 0,
        stdev: 0,
        median: 0,
        p90: 0,
        max: 0,
        count: 0,
        rate_count: 0,
        count_1xx: 0,
        rate_1xx: 0,
        count_2xx: 0,
        rate_2xx: 0,
        count_3xx: 0,
        rate_3xx: 0,
        count_4xx: 0,
        rate_4xx: 0,
        count_5xx: 0,
        rate_5xx: 0,
    };
    let reducer = (acc, s) => {
        if (!s) {
            return acc;
        }

        if (acc.min === undefined || s.min < acc.min) {
            acc.min = s.min;
        }
        if (s.max > acc.max) {
            acc.max = s.max;
        }
        acc.count += s.count;
        acc.rate_count += s.rate_count;
        acc.count_1xx += s.count_1xx;
        acc.rate_1xx += s.rate_1xx;
        acc.count_2xx += s.count_2xx;
        acc.rate_2xx += s.rate_2xx;
        acc.count_3xx += s.count_3xx;
        acc.rate_3xx += s.rate_3xx;
        acc.count_4xx += s.count_4xx;
        acc.rate_4xx += s.rate_4xx;
        acc.count_5xx += s.count_5xx;
        acc.rate_5xx += s.rate_5xx;
        acc.count_fail += s.count_fail;
        acc.rate_fail += s.rate_fail;
        acc.mean += s.mean;
        acc.stdev += s.stdev;
        acc.median += s.median;
        acc.p90 += s.p90;
        return acc;
    };
    let data = stats
        .filter(s => s.state !== "Disconnected")
        .map(s => s.latest || undefined)
        .reduce(reducer, initial);
    let datapoints = stats.filter(s => s.state !== "Disconnected").length;
    if (datapoints > 0) {
        data['mean'] = data['mean'] / datapoints;
        data['stdev'] = data['stdev'] / datapoints;
        data['median'] = data['median'] / datapoints;
        data['p90'] = data['p90'] / datapoints;
    }
    return (<tr className="footer-row">
                <td colspan="4"></td>
                <td><Gauge value={data.min}/></td>
                <td><Gauge value={data.mean.toFixed(1)}/></td>
                <td><Gauge value={data.stdev.toFixed(1)}/></td>
                <td><Gauge value={data.median.toFixed(1)}/></td>
                <td><Gauge value={data.p90.toFixed(1)}/></td>
                <td><Gauge value={data.max}/></td>
                <td><Meter rate={data.rate_count} count={data.count}/></td>
                <td><Meter rate={data.rate_1xx} count={data.count_1xx}/></td>
                <td>
                    <Meter rate={data.rate_2xx} count={data.count_2xx} />
                </td>
                <td>
                    <Meter rate={data.rate_3xx} count={data.count_3xx} />
                </td>
                <td>
                    <Meter rate={data.rate_4xx} count={data.count_4xx}/>
                </td>
                <td>
                    <Meter rate={data.rate_5xx} count={data.count_5xx}/>
                </td>
                <td>
                    <Meter rate={data.rate_fail} count={data.count_fail}/>
                </td>
                <td></td>
            </tr>);
}

const WorkerTable = {
    oninit: (vnode) => {
        StatsApi.fetch();
        let interval = setInterval(StatsApi.fetch, 2000);
        vnode.state.interval = interval;
    },
    view:() => {
        return (<table className="worker-table">
                    <thead>
                        <tr>
                            <th colspan="4"></th>
                            <th colspan="6">Response Times (ms)</th>
                            <th colspan="7">Request Counts</th>
                        </tr>
                        <tr className="header-row">
                            <th>ID</th>
                            <th>Host</th>
                            <th>State</th>
                            <th>Elapsed</th>
                            <th>Min</th>
                            <th>Mean</th>
                            <th>Stdev</th>
                            <th>Median</th>
                            <th>p90</th>
                            <th>Max</th>
                            <th>Requests</th>
                            <th>1xx</th>
                            <th className="bg-green">2xx</th>
                            <th>3xx</th>
                            <th className="bg-yellow">4xx</th>
                            <th className="bg-red">5xx</th>
                            <th className="bg-red">Fail</th>
                            <th>Available/Queue Depth/Max</th>
                        </tr>
                    </thead>
                    <tbody>
                        { StatsApi.data.map(makeWorkerRow) }
                    </tbody>
                    <tfoot>
                        { makeSummaryRow(StatsApi.data) }
                    </tfoot>
                </table>);
    }
};

export default WorkerTable;
