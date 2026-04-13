function getCSSVar(name) {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function buildCytoscapeStyle() {
    return [
        { selector: 'node', style: {
            'label': 'data(label)',
            'font-size': '10px',
            'color': getCSSVar('--text-primary'),
            'text-valign': 'center',
            'text-halign': 'center',
            'background-color': getCSSVar('--accent'),
            'shape': 'roundrectangle',
            'width': 'label',
            'height': 'label',
            'padding': '6px',
        }},
        { selector: 'node[kind="function"]', style: { 'background-color': getCSSVar('--accent') }},
        { selector: 'node[kind="struct"]',   style: { 'background-color': getCSSVar('--success') }},
        { selector: 'node[kind="class"]',    style: { 'background-color': getCSSVar('--purple') }},
        { selector: 'node[kind="method"]',   style: { 'background-color': getCSSVar('--warning') }},
        { selector: 'node[kind="interface"]',style: { 'background-color': getCSSVar('--danger') }},
        { selector: ':parent', style: {
            'background-color': getCSSVar('--bg-surface'),
            'border-color': getCSSVar('--border'),
            'border-width': 1,
            'label': 'data(label)',
            'font-size': '8px',
            'text-valign': 'top',
            'color': getCSSVar('--text-muted'),
        }},
        { selector: 'edge', style: {
            'width': 1,
            'line-color': getCSSVar('--border'),
            'target-arrow-color': getCSSVar('--border'),
            'target-arrow-shape': 'triangle',
            'curve-style': 'bezier',
            'arrow-scale': 0.8,
        }},
        { selector: 'edge[kind="calls"]',          style: { 'line-color': getCSSVar('--accent'),    'target-arrow-color': getCSSVar('--accent') }},
        { selector: 'edge[kind="imports"]',         style: { 'line-color': getCSSVar('--text-muted'), 'target-arrow-color': getCSSVar('--text-muted') }},
        { selector: 'edge[kind="type_reference"]',  style: { 'line-color': getCSSVar('--purple'),    'target-arrow-color': getCSSVar('--purple') }},
    ];
}

document.addEventListener('DOMContentLoaded', async () => {
    const res = await fetch('/api/graph?limit=200');
    const data = await res.json();

    const elements = [];
    data.nodes.forEach(n => elements.push({ group: 'nodes', data: n.data }));
    data.edges.forEach(e => elements.push({ group: 'edges', data: e.data }));

    const cy = cytoscape({
        container: document.getElementById('cy'),
        elements: elements,
        style: buildCytoscapeStyle(),
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

    // Watch for theme changes and refresh Cytoscape styles
    const observer = new MutationObserver(() => {
        cy.style(buildCytoscapeStyle());
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
});

function showNodeDetail(detail) {
    const panel = document.getElementById('node-detail');
    panel.classList.add('open');
    panel.innerHTML = `
        <div class="side-panel-header">
            <h3 style="margin:0">${detail.name}</h3>
            <button onclick="this.closest('.side-panel').classList.remove('open')">&times;</button>
        </div>
        <div class="side-panel-body">
            <span class="badge badge-${detail.kind}">${detail.kind}</span>
            <p class="text-muted" style="margin:8px 0">${detail.file}:${detail.lines[0]}-${detail.lines[1]}</p>
            <pre style="font-size:0.85em"><code>${detail.signature}</code></pre>
            ${detail.outgoing.length > 0 ? '<h4>Calls</h4><ul>' + detail.outgoing.map(d => `<li>${d.name} <span class="text-muted">${d.file}</span></li>`).join('') + '</ul>' : ''}
            ${detail.incoming.length > 0 ? '<h4>Called By</h4><ul>' + detail.incoming.map(d => `<li>${d.name} <span class="text-muted">${d.file}</span></li>`).join('') + '</ul>' : ''}
        </div>
    `;
}
