pub fn base(title: &str, active_page: &str, content: &str) -> String {
    base_with_scripts(title, active_page, content, "")
}

pub fn base_with_scripts(title: &str, active_page: &str, content: &str, extra_scripts: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title} — codeindex</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;700&display=swap" rel="stylesheet">
    <script>(function(){{var t=localStorage.getItem('codeindex-theme');if(!t){{t=window.matchMedia('(prefers-color-scheme:dark)').matches?'dark':'light'}}document.documentElement.setAttribute('data-theme',t);}})()</script>
    <link rel="stylesheet" href="/assets/css/style.css">
    <link rel="stylesheet" href="/assets/css/hljs-dark.css">
    <script src="/assets/js/htmx.min.js"></script>
    <script src="/assets/js/theme.js"></script>
</head>
<body>
    <nav class="sidebar">
        <div class="sidebar-header">
            <h1 class="logo">codeindex</h1>
            <p class="logo-subtitle">code intelligence</p>
        </div>
        <div class="nav-section-label">NAVIGATION</div>
        <ul class="nav-links">
            <li>
                <a href="/" class="{dashboard_active}">
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="1" y="1" width="6" height="6" rx="1"/><rect x="9" y="1" width="6" height="6" rx="1"/><rect x="1" y="9" width="6" height="6" rx="1"/><rect x="9" y="9" width="6" height="6" rx="1"/></svg>
                    Dashboard
                </a>
            </li>
            <li>
                <a href="/search" class="{search_active}">
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="6.5" cy="6.5" r="4.5"/><line x1="10.5" y1="10.5" x2="14" y2="14"/></svg>
                    Search
                </a>
            </li>
            <li>
                <a href="/files" class="{files_active}">
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M3 2h6l4 4v8a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1z"/><polyline points="9,2 9,6 13,6"/></svg>
                    Files
                </a>
            </li>
            <li>
                <a href="/graph" class="{graph_active}">
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="3" cy="8" r="2"/><circle cx="13" cy="3" r="2"/><circle cx="13" cy="13" r="2"/><line x1="5" y1="8" x2="11" y2="4"/><line x1="5" y1="8" x2="11" y2="12"/></svg>
                    Graph
                </a>
            </li>
        </ul>
        <div class="nav-section-label">TOOLS</div>
        <ul class="nav-links">
            <li>
                <a href="/activity" class="{activity_active}">
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="8" cy="8" r="6"/><polyline points="8,5 8,8 10,10"/></svg>
                    Activity
                </a>
            </li>
            <li>
                <a href="/settings" class="{settings_active}">
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="8" cy="8" r="2.5"/><path d="M8 1v2M8 13v2M1 8h2M13 8h2M3.05 3.05l1.41 1.41M11.54 11.54l1.41 1.41M3.05 12.95l1.41-1.41M11.54 4.46l1.41-1.41"/></svg>
                    Settings
                </a>
            </li>
        </ul>
        <div class="sidebar-footer">
            <div class="theme-toggle">
                <button class="theme-btn" onclick="setTheme('light')" title="Light theme">
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="8" cy="8" r="3"/><line x1="8" y1="1" x2="8" y2="3"/><line x1="8" y1="13" x2="8" y2="15"/><line x1="1" y1="8" x2="3" y2="8"/><line x1="13" y1="8" x2="15" y2="8"/><line x1="2.93" y1="2.93" x2="4.34" y2="4.34"/><line x1="11.66" y1="11.66" x2="13.07" y2="13.07"/><line x1="2.93" y1="13.07" x2="4.34" y2="11.66"/><line x1="11.66" y1="4.34" x2="13.07" y2="2.93"/></svg>
                </button>
                <button class="theme-btn" onclick="setTheme('dark')" title="Dark theme">
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M14 10A6 6 0 0 1 6 2a6 6 0 1 0 8 8z"/></svg>
                </button>
                <button class="theme-btn" onclick="setTheme('system')" title="System theme">
                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="1" y="2" width="14" height="10" rx="1"/><line x1="5" y1="14" x2="11" y2="14"/><line x1="8" y1="12" x2="8" y2="14"/></svg>
                </button>
            </div>
        </div>
    </nav>
    <main class="content">
        {content}
    </main>
    {extra_scripts}
</body>
</html>"#,
        title = title,
        content = content,
        extra_scripts = extra_scripts,
        dashboard_active = if active_page == "dashboard" { "active" } else { "" },
        search_active = if active_page == "search" { "active" } else { "" },
        files_active = if active_page == "files" { "active" } else { "" },
        graph_active = if active_page == "graph" { "active" } else { "" },
        activity_active = if active_page == "activity" { "active" } else { "" },
        settings_active = if active_page == "settings" { "active" } else { "" },
    )
}
