function getCSSVar(name) {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

let langChartInstance = null;
let kindChartInstance = null;
let lastStatsData = null;

function buildCharts(data) {
    // Destroy existing chart instances before rebuilding
    if (langChartInstance) {
        langChartInstance.destroy();
        langChartInstance = null;
    }
    if (kindChartInstance) {
        kindChartInstance.destroy();
        kindChartInstance = null;
    }

    const accent      = getCSSVar('--accent');
    const success     = getCSSVar('--success');
    const purple      = getCSSVar('--purple');
    const warning     = getCSSVar('--warning');
    const danger      = getCSSVar('--danger');
    const textMuted   = getCSSVar('--text-muted');
    const textPrimary = getCSSVar('--text-primary');
    const border      = getCSSVar('--border');

    // Language bar chart
    const langCanvas = document.getElementById('lang-chart');
    if (langCanvas && data.languages && data.languages.length > 0) {
        // Restore canvas if it was replaced with a message
        if (langCanvas.tagName !== 'CANVAS') return;
        const ctx = langCanvas.getContext('2d');
        langChartInstance = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: data.languages.map(l => l.label),
                datasets: [{
                    label: 'Files',
                    data: data.languages.map(l => l.count),
                    backgroundColor: accent,
                    borderRadius: 4,
                }]
            },
            options: {
                indexAxis: 'y',
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: { display: false }
                },
                scales: {
                    x: {
                        grid: { color: border },
                        ticks: { color: textMuted }
                    },
                    y: {
                        grid: { display: false },
                        ticks: { color: textPrimary }
                    }
                }
            }
        });
    } else if (langCanvas) {
        langCanvas.parentElement.innerHTML = '<p class="text-muted" style="text-align:center;padding:40px 0;">No files indexed yet</p>';
    }

    // Region kind doughnut chart
    const kindCanvas = document.getElementById('kind-chart');
    if (kindCanvas && data.region_kinds && data.region_kinds.length > 0) {
        const cssColorVars = ['--accent', '--success', '--purple', '--warning', '--danger', '--text-muted'];
        const colors = cssColorVars.map(v => getCSSVar(v));
        // Extend with fallback repeats if there are more slices than CSS vars
        while (colors.length < data.region_kinds.length) {
            colors.push(...cssColorVars.map(v => getCSSVar(v)));
        }

        const ctx2 = kindCanvas.getContext('2d');
        kindChartInstance = new Chart(ctx2, {
            type: 'doughnut',
            data: {
                labels: data.region_kinds.map(r => r.label),
                datasets: [{
                    data: data.region_kinds.map(r => r.count),
                    backgroundColor: colors.slice(0, data.region_kinds.length),
                    borderWidth: 0,
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        position: 'right',
                        labels: {
                            color: textPrimary,
                            padding: 12,
                            boxWidth: 12,
                        }
                    }
                }
            }
        });
    } else if (kindCanvas) {
        kindCanvas.parentElement.innerHTML = '<p class="text-muted" style="text-align:center;padding:40px 0;">No regions indexed yet</p>';
    }
}

document.addEventListener('DOMContentLoaded', async () => {
    let data;
    try {
        const res = await fetch('/api/stats');
        data = await res.json();
    } catch (e) {
        console.error('Failed to fetch stats:', e);
        return;
    }

    lastStatsData = data;

    // Update stat cards
    const filesEl   = document.getElementById('stat-files');
    const regionsEl = document.getElementById('stat-regions');
    const depsEl    = document.getElementById('stat-deps');
    const sizeEl    = document.getElementById('stat-size');

    if (filesEl)   filesEl.textContent   = data.total_files.toLocaleString();
    if (regionsEl) regionsEl.textContent = data.total_regions.toLocaleString();
    if (depsEl)    depsEl.textContent    = data.total_dependencies.toLocaleString();
    if (sizeEl) {
        const totalBytes = data.db_size_bytes + data.vector_size_bytes;
        if (totalBytes >= 1024 * 1024) {
            sizeEl.textContent = (totalBytes / (1024 * 1024)).toFixed(1) + ' MB';
        } else {
            sizeEl.textContent = (totalBytes / 1024).toFixed(1) + ' KB';
        }
    }

    // Last indexed timestamp
    const lastEl = document.getElementById('last-indexed');
    if (lastEl && data.last_indexed_at) {
        lastEl.textContent = 'Last indexed: ' + timeAgo(data.last_indexed_at * 1000);
    } else if (lastEl) {
        lastEl.textContent = 'Not yet indexed';
    }

    buildCharts(data);

    // Rebuild charts whenever the theme (data-theme attribute) changes
    const observer = new MutationObserver((mutations) => {
        for (const mutation of mutations) {
            if (mutation.type === 'attributes' && mutation.attributeName === 'data-theme') {
                if (lastStatsData) buildCharts(lastStatsData);
                break;
            }
        }
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
});

function timeAgo(timestamp) {
    const seconds = Math.floor((Date.now() - timestamp) / 1000);
    if (seconds < 60) return seconds + 's ago';
    if (seconds < 3600) return Math.floor(seconds / 60) + 'm ago';
    if (seconds < 86400) return Math.floor(seconds / 3600) + 'h ago';
    return Math.floor(seconds / 86400) + 'd ago';
}
