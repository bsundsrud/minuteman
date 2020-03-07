import m from 'mithril';
import './SpecList.css';

function SpecListEntryBody(initial) {
    return {
        view: (vnode) => {
            return (
                <div className="spec-list-body">
                    <h4 className="spec-list-body-header">Body:</h4>
                    <pre className="spec-list-body-text">{ vnode.attrs.body }</pre>
                </div>
            );
        },
    };
}

function SpecListEntry(initial) {
    var expanded = false;

    const expandButtonCss = () => {
        return expanded ?
            "spec-list-expand spec-list-expanded" :
            "spec-list-expand spec-list-collapsed";
    };

    const extraSectionCss = () => {
        return expanded ? "spec-list-extra" : "spec-list-extra hidden";
    };

    function displayVersion(versionId) {
        if (versionId === "Http11") {
            return "HTTP/1.1";
        } else if (versionId === "Http2") {
            return "HTTP/2";
        } else {
            return versionId;
        }
    }

    return {
        view: (vnode) => {
            let spec = vnode.attrs.spec;
            return (
                <div className="spec-list-entry">
                    <a className="spec-list-btn" onclick={ vnode.attrs.specDeleted }>X</a>
                    <span className="spec-list-method">{ spec.method }</span>
                    <span className="spec-list-url">{ spec.url }</span>
                    <span className="spec-list-version">{ displayVersion(spec.version) }</span>
                    <a className={ expandButtonCss() } onclick={ (e) => expanded = !expanded }></a>
                    <div className={ extraSectionCss() }>
                        <h4 className="spec-list-header-list-header">Headers:</h4>
                        { spec.headers.length > 0 ?
                          (<dl className="spec-list-header-list">
                               { Object.keys(spec.headers).map((key) => {
                                   return (
                                       <>
                                           <dt>{ key }</dt>
                                           <dd>{ spec.headers[key] }</dd>
                                       </>
                                   );
                               }) }
                           </dl>)
                          : (<span className="spec-list-empty">None</span>) }
                        { spec.body ?
                          (<SpecListEntryBody body={ spec.body }/>): null }
                    </div>

                </div>
            );
        },
    };
}

function SpecList(initialVnode) {

    return {
        view: (vnode) => {
            function specDeleted(index) {
                return (e) => {
                    vnode.attrs.specs.splice(index, 1);
                };
            }
            return (
                <section className="spec-list">
                    <h3 className="spec-list-header">Specs</h3>
                    <div className="spec-list-entry-container">
                        { vnode.attrs.specs.length > 0 ?
                          vnode.attrs.specs.map((spec, index) => <SpecListEntry
                                                                     spec={ spec }
                                                                     specDeleted={ specDeleted(index) }
                                                                 />)
                          : (<span className="spec-list-empty">None</span>) }
                    </div>
                </section>
            );
        },
    };
}

export default SpecList;
