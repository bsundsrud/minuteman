import m from 'mithril';

const StatsApi = {
    fetching: false,
    data: [],
    fetch: function() {
        StatsApi.fetching = true;
        m.request({
            method: "GET",
            url: "/stats",
        }).then((resp) => {
            let items = resp.items;
            items.sort((first, second) => first.id - second.id);
            StatsApi.data = items;
            StatsApi.fetching = false;
        });
    }
};

const ControlsApi = {
    specs: [],
    strategy: "Random",
    concurrency: 100,
    addSpec: (spec) => {
        ControlsApi.specs.push(spec);
    },
    deleteSpec: (index) => {
        ControlsApi.specs.splice(index, 1);
    },
    setStrategy: (strategy) => {
        ControlsApi.strategy = strategy;
    },
    setConcurrency: (concurrency) => {
        ControlsApi.concurrency = parseInt(concurrency, 10);
    },
    start: () => {
        let req = {
            requests: ControlsApi.specs,
            strategy: ControlsApi.strategy,
            max_concurrency: ControlsApi.concurrency,
        };
        m.request({
            method: "POST",
            url: "/workers/start",
            body: req,
        });
    },
    stop: () => {
        m.request({
            method: "POST",
            url: "/workers/stop"
        });
    },
    reset: () => {
        m.request({
            method: "POST",
            url: "/workers/reset"
        });
    },
};

export {
    StatsApi,
    ControlsApi
};
