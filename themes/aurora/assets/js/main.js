(function() {
  'use strict';

  // 暗黑模式切换
  function getPreferredTheme() {
    var stored = localStorage.getItem('theme');
    if (stored) return stored;
    var dataTheme = document.body.getAttribute('data-theme');
    if (dataTheme && dataTheme !== 'auto') return dataTheme;
    return 'auto';
  }

  function applyTheme(theme) {
    if (theme === 'auto') {
      document.body.removeAttribute('data-theme');
    } else {
      document.body.setAttribute('data-theme', theme);
    }
  }

  function cycleTheme() {
    var current = getPreferredTheme();
    var next;
    if (current === 'auto') {
      next = 'dark';
    } else if (current === 'dark') {
      next = 'light';
    } else {
      next = 'auto';
    }
    localStorage.setItem('theme', next);
    applyTheme(next);
    updateToggleIcon(next);
  }

  function updateToggleIcon(theme) {
    var btn = document.querySelector('.theme-toggle');
    if (!btn) return;
    if (theme === 'dark') {
      btn.textContent = '☾';
    } else if (theme === 'light') {
      btn.textContent = '☀';
    } else {
      btn.textContent = '◐';
    }
  }

  // 移动端汉堡菜单
  function initMobileNav() {
    var toggle = document.querySelector('.nav-toggle');
    var links = document.querySelector('.nav-links');
    if (!toggle || !links) return;
    toggle.addEventListener('click', function() {
      links.classList.toggle('active');
    });
  }

  // 初始化
  var theme = getPreferredTheme();
  applyTheme(theme);
  updateToggleIcon(theme);

  document.addEventListener('DOMContentLoaded', function() {
    var toggleBtn = document.querySelector('.theme-toggle');
    if (toggleBtn) {
      toggleBtn.addEventListener('click', cycleTheme);
    }
    initMobileNav();
  });
})();
