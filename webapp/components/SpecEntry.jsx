import m from 'mithril';
import './SpecEntry.css';

const possibleMethods = [
    {id: "GET", body: false },
    {id: "POST", body: true},
    {id: "PUT", body: true},
    {id: "DELETE", body: false},
    {id: "HEAD", body: false},
    {id: "OPTIONS", body: false},
    {id: "CONNECT", body: false},
    {id: "PATCH", body: true},
    {id: "TRACE", body: false}
];

const possibleVersions = [
    {id: "Http11", display: "HTTP/1.1"},
    {id: "Http2", display: "HTTP/2"},
];

function urlEntry(selectedMethod, url, selectedVersion, methodChanged, urlChanged, versionChanged) {
    return (
        <section className="url-entry">
            <select className="method-select" onchange={methodChanged}>
                { possibleMethods.map((method, index) => {
                    return (
                        <option value={ index } selected={ index === selectedMethod }>
                            { method.id }
                        </option>
                    );
                }) }
            </select>
            <input className="url-input" type="text" value={url} oninput={urlChanged} placeholder="Url (ex. https://example.com)" />
            <select className="version-select" onchange={versionChanged}>
                { possibleVersions.map((version, index) => {
                    return (
                        <option value={ index } selected={ index === selectedVersion }>
                            { version.display }
                        </option>
                    );
                }) }
            </select>
        </section>
    );
}

function headerCallback(f, key, val) {
    return (e) => f(e, key, val);
}

function HeaderEntry(initial) {
    function headerAction(f, type, headers, index, key, value) {
        return (e) => {
            if (type === "edit-key") {
                headers[index] = [e.target.value, value];
                return f(headers);
            } else if (type === "edit-value") {
                headers[index] = [key, e.target.value];
                return f(headers);
            } else if (type === "add") {
                headers.push([key, value]);
                return f(headers);
            } else if (type === "remove") {
                headers.splice(index, 1);
                return f(headers);

            }
        };
    }

    var newHeaderKey = "";
    var newHeaderVal = "";
    var finishedAdding = false;

    const clearAfter = (f) => (e) => {
        let val = f(e);
        newHeaderKey = "";
        newHeaderVal = "";
        return val;
    };

    const execIfEnter = (f) => (e) => {
        if (e.key === "Enter") {
            f(e);
            finishedAdding = true;
        }
    };
    return {
        onupdate: (vnode) => {
            if (finishedAdding === true) {
                finishedAdding = false;
                headerEntryInput.focus();
            }
        },
        view: (vnode) => {
            let addFunc = clearAfter(headerAction(vnode.attrs.headersChanged, "add", vnode.attrs.headers, null, newHeaderKey, newHeaderVal))
            return (
                <section className="header-entry">
                    <h4 className="header-entry-header">Headers:</h4>
                    <div class="header-entry-list">
                        { vnode.attrs.headers.length === 0 ? (
                            <div className="header-entry-line">
                                <span className="spec-list-empty">None</span>
                            </div>
                        ) : null }
                        { vnode.attrs.headers.map((header, index) => {
                            let key = header[0];
                            let value = header[1];
                            return (
                                <div className="header-entry-line">
                                    <input type="text"
                                           value={key}
                                           className="header-key-input"
                                           oninput={ headerAction(vnode.attrs.headersChanged, "edit-key", vnode.attrs.headers, index, key, value) }
                                    />
                                    <span className="header-entry-line-spacer">:</span>
                                    <input type="text"
                                           className="header-value-input"
                                           value={value}
                                           oninput={ headerAction(vnode.attrs.headersChanged, "edit-value", vnode.attrs.headers, index, key, value) }
                                    />
                                    <a
                                           className="spec-list-btn header-entry-remove-btn"
                                           onclick={ headerAction(vnode.attrs.headersChanged, "remove", vnode.attrs.headers, index, key, value) }
                                    >-</a>
                                </div>
                            );
                        }) }
                        <div className="header-entry-new">
                            <input type="text"
                                   id="headerEntryInput"
                                   className="header-key-input"
                                   oninput={ (e) => newHeaderKey = e.target.value }
                                   onkeypress={ execIfEnter(addFunc) }
                                   value={ newHeaderKey }
                                   placeholder="Header Name"
                            />
                            <span className="header-entry-line-spacer">:</span>
                            <input type="text"
                                   className="header-value-input"
                                   oninput={ (e) => newHeaderVal = e.target.value }
                                   onkeypress={ execIfEnter(addFunc) }
                                   value={ newHeaderVal }
                                   placeholder="Header Value"
                            />
                            <a className="spec-list-btn header-entry-add-btn" onclick={ addFunc }>+</a>
                        </div>
                    </div>
                </section>

            );
        },
    };
}

function bodyEntry(body, bodyChanged) {
    return (
        <section className="body-entry">
            <h4 className="body-entry-header">Body</h4>
            <textarea className="body-entry-input" oninput={ bodyChanged }>{ body }</textarea>
        </section>
    );
}

function SpecEntry(initial) {
    var method = 0;
    var url = "";
    var body = "";
    var headers = [];
    var version = 0;

    function methodChanged(e) {
        method = parseInt(e.target.value, 10);
    }

    function urlChanged(e) {
        url = e.target.value;
    }

    function versionChanged(e) {
        version = parseInt(e.target.value, 10);
    }

    function headersChanged(newHeaders) {
        headers = newHeaders;
    }

    function bodyChanged(e) {
        body = e.target.value;
    }

    function createSpec(callback) {
        return (e) => {
            if (url === "") {
                return;
            }
            const spec = {
                method: possibleMethods[method].id,
                url: url,
                version: possibleVersions[version].id,
                headers: headers.reduce((acc, val) => {
                    acc[val[0]] = val[1];
                    return acc;
                }, {}),
                body: possibleMethods[method].body ? body : null,
            };
            clear();
            callback(spec);
        };
    }

    function clear() {
        method = 0;
        url = "";
        body = "";
        headers = [];
        version = 0;
    }

    return {
        view: (vnode) => {
            return (
                <section className="spec-entry">
                    <div className="entry-container">
                        { urlEntry(method, url, version, methodChanged, urlChanged, versionChanged) }
                        <HeaderEntry headers={headers} headersChanged={headersChanged} />
                        { possibleMethods[method].body ? bodyEntry(body, bodyChanged) : null }
                    </div>
                    <a className="spec-list-btn spec-entry-add-btn" onclick={ createSpec(vnode.attrs.specCreated) }>Add Spec</a>
                </section>
            );
        },
    };
}

export default SpecEntry;
