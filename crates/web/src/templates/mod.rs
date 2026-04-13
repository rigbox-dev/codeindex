pub fn base(title: &str, active_page: &str, content: &str) -> String {
    base_with_scripts(title, active_page, content, "")
}

pub fn base_with_scripts(title: &str, active_page: &str, content: &str, extra_scripts: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title} — codeindex</title>
    <link rel="stylesheet" href="/assets/css/style.css">
    <script src="/assets/js/htmx.min.js"></script>
</head>
<body>
    <nav class="sidebar">
        <div class="sidebar-header">
            <h1 class="logo">codeindex</h1>
        </div>
        <ul class="nav-links">
            <li><a href="/" class="{dashboard_active}">Dashboard</a></li>
            <li><a href="/search" class="{search_active}">Search</a></li>
            <li><a href="/files" class="{files_active}">Files</a></li>
            <li><a href="/graph" class="{graph_active}">Graph</a></li>
            <li><a href="/activity" class="{activity_active}">Activity</a></li>
            <li><a href="/settings" class="{settings_active}">Settings</a></li>
        </ul>
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
