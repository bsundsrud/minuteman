import m from 'mithril';
import SpecEntry from './SpecEntry';
import SpecList from './SpecList';
import './SpecEditor.css';

function SpecEditor(initialVnode) {
    return {
        view: (vnode) => {
            function specCreated(spec) {
                vnode.attrs.specs.push(spec);
            }
            return (
                <section className="spec-editor">
                    <SpecList specs={ vnode.attrs.specs }/>
                    <SpecEntry specCreated={ specCreated }/>
                </section>
            );
        },
    };
}

export default SpecEditor;
