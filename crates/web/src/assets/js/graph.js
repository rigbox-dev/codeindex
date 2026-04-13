document.addEventListener('DOMContentLoaded', async () => {
    const res = await fetch('/api/graph?limit=200');
    const data = await res.json();

    const elements = [];
    data.nodes.forEach(n => elements.push({ group: 'nodes', data: n.data }));
    data.edges.forEach(e => elements.push({ group: 'edges', data: e.data }));

    const cy = cytoscape({
        container: document.getElementById('cy'),
        elements: elements,
        style: [
            { selector: 'node', style: {
                'label': 'data(label)',
                'font-size': '10px',
                'color': '#e6edf3',
                'text-valign': 'center',
                'text-halign': 'center',
                'background-color': '#58a6ff',
                'shape': 'roundrectangle',
                'width': 'label',
                'height': 'label',
                'padding': '6px',
            }},
            { selector: 'node[kind="function"]', style: { 'background-color': '#58a6ff' }},
            { selector: 'node[kind="struct"]', style: { 'background-color': '#3fb950' }},
            { selector: 'node[kind="class"]', style: { 'background-color': '#bc8cff' }},
            { selector: 'node[kind="method"]', style: { 'background-color': '#d29922' }},
            { selector: 'node[kind="interface"]', style: { 'background-color': '#f85149' }},
            { selector: ':parent', style: {
                'background-color': '#161b22',
                'border-color': '#30363d',
                'border-width': 1,
                'label': 'data(label)',
                'font-size': '8px',
                'text-valign': 'top',
                'color': '#8b949e',
            }},
            { selector: 'edge', style: {
                'width': 1,
                'line-color': '#30363d',
                'target-arrow-color': '#30363d',
                'target-arrow-shape': 'triangle',
                'curve-style': 'bezier',
                'arrow-scale': 0.8,
            }},
            { selector: 'edge[kind="calls"]', style: { 'line-color': '#58a6ff', 'target-arrow-color': '#58a6ff' }},
            { selector: 'edge[kind="imports"]', style: { 'line-color': '#8b949e', 'target-arrow-color': '#8b949e' }},
            { selector: 'edge[kind="type_reference"]', style: { 'line-color': '#bc8cff', 'target-arrow-color': '#bc8cff' }},
        ],
        layout: { name: 'cose', animate: false, nodeOverlap: 20, idealEdgeLength: 100 },
    });

    // Node click -> show detail
    cy.on('tap', 'node', async (evt) => {
        const node = evt.target;
        const id = node.data('id');
        if (id.startsWith('f_')) return; // skip file compound nodes
        const regionId = id.replace('r_', '');
        const res = await fetch('/api/graph/node/' + regionId);
        const detail = await res.json();
        showNodeDetail(detail);
    });

    // Layout selector
    document.getElementById('layout-select')?.addEventListener('change', (e) => {
        cy.layout({ name: e.target.value, animate: true, animationDuration: 500 }).run();
    });

    document.getElementById('fit-btn')?.addEventListener('click', () => cy.fit());
});

function showNodeDetail(detail) {
    const panel = document.getElementById('node-detail');
    panel.classList.add('open');
    panel.innerHTML = `
        <div style="padding:20px">
            <div style="display:flex;justify-content:space-between;align-items:center">
                <h3 style="margin:0">${detail.name}</h3>
                <button onclick="this.closest('.side-panel').classList.remove('open')" style="background:none;border:none;color:#8b949e;cursor:pointer;font-size:1.2em">&times;</button>
            </div>
            <span class="badge badge-${detail.kind}">${detail.kind}</span>
            <p style="color:#8b949e;margin:8px 0">${detail.file}:${detail.lines[0]}-${detail.lines[1]}</p>
            <pre style="font-size:0.85em"><code>${detail.signature}</code></pre>
            ${detail.outgoing.length > 0 ? '<h4>Calls</h4><ul>' + detail.outgoing.map(d => `<li>${d.name} <span style="color:#8b949e">${d.file}</span></li>`).join('') + '</ul>' : ''}
            ${detail.incoming.length > 0 ? '<h4>Called By</h4><ul>' + detail.incoming.map(d => `<li>${d.name} <span style="color:#8b949e">${d.file}</span></li>`).join('') + '</ul>' : ''}
        </div>
    `;
}
