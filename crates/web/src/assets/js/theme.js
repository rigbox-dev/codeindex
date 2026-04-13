(function() {
    function getTheme() {
        var stored = localStorage.getItem('codeindex-theme');
        if (stored === 'light' || stored === 'dark') return stored;
        return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }

    function applyTheme(theme) {
        document.documentElement.setAttribute('data-theme', theme);
        // Update toggle button states
        document.querySelectorAll('[data-theme-btn]').forEach(function(btn) {
            btn.classList.toggle('active', btn.getAttribute('data-theme-btn') === theme ||
                (btn.getAttribute('data-theme-btn') === 'system' && !localStorage.getItem('codeindex-theme')));
        });
    }

    window.setTheme = function(mode) {
        if (mode === 'system') {
            localStorage.removeItem('codeindex-theme');
            applyTheme(window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
        } else {
            localStorage.setItem('codeindex-theme', mode);
            applyTheme(mode);
        }
    };

    // Listen for system preference changes
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function() {
        if (!localStorage.getItem('codeindex-theme')) {
            applyTheme(window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
        }
    });

    // Apply on load
    applyTheme(getTheme());
})();
