import * as m from 'mithril';

let item = {
    view: function() {
        return m("div.test");
    }
};

m.mount(document.body, item);
