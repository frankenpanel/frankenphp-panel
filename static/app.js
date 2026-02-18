(function () {
  'use strict';

  var toastContainer = document.getElementById('toast-container');
  if (!toastContainer) return;

  function showToast(message, type) {
    type = type || 'success';
    var toast = document.createElement('div');
    toast.setAttribute('role', 'alert');
    toast.className = type === 'success'
      ? 'rounded-lg bg-emerald-600 text-white px-4 py-3 text-sm font-medium shadow-lg ring-1 ring-black/5'
      : 'rounded-lg bg-red-600 text-white px-4 py-3 text-sm font-medium shadow-lg ring-1 ring-black/5';
    toast.textContent = message;
    toastContainer.appendChild(toast);
    setTimeout(function () {
      toast.style.opacity = '0';
      toast.style.transform = 'translateX(0.5rem)';
      toast.style.transition = 'opacity 0.2s, transform 0.2s';
      setTimeout(function () { toast.remove(); }, 200);
    }, 4000);
  }

  function parseQuery() {
    var q = {};
    (window.location.search || '').slice(1).split('&').forEach(function (part) {
      var p = part.split('=');
      if (p[0]) q[decodeURIComponent(p[0])] = decodeURIComponent((p[1] || '').replace(/\+/g, ' '));
    });
    return q;
  }

  var query = parseQuery();
  if (query.created === '1') showToast('Site created successfully.', 'success');
  if (query.deleted === '1') showToast('Site deleted.', 'success');
  if (query.db_created === '1') showToast('Database created successfully.', 'success');
  if (query.restarted === '1') showToast('Site restart requested.', 'success');

  var loginForm = document.getElementById('login-form');
  if (loginForm) {
    loginForm.addEventListener('submit', function () {
      var btn = document.getElementById('login-btn');
      if (btn) btn.classList.add('loading');
    });
  }

  var addSiteForm = document.getElementById('add-site-form');
  if (addSiteForm) {
    addSiteForm.addEventListener('submit', function () {
      var btn = document.getElementById('add-site-btn');
      if (btn) btn.classList.add('loading');
    });
  }

  var addDbForm = document.getElementById('add-database-form');
  if (addDbForm) {
    addDbForm.addEventListener('submit', function () {
      var btn = document.getElementById('add-db-btn');
      if (btn) btn.classList.add('loading');
    });
  }

  var searchInput = document.getElementById('search-sites');
  var sitesTable = document.getElementById('sites-table');
  if (searchInput && sitesTable) {
    var rows = sitesTable.querySelectorAll('tbody tr');
    searchInput.addEventListener('input', function () {
      var term = (searchInput.value || '').toLowerCase().trim();
      rows.forEach(function (row) {
        var domain = (row.getAttribute('data-domain') || '').toLowerCase();
        var pathEl = row.querySelector('.path');
        var path = pathEl ? pathEl.textContent : '';
        var show = !term || domain.indexOf(term) !== -1 || path.toLowerCase().indexOf(term) !== -1;
        row.style.display = show ? '' : 'none';
      });
    });
  }

  document.querySelectorAll('.btn-delete-site').forEach(function (btn) {
    btn.addEventListener('click', function () {
      var id = btn.getAttribute('data-id');
      var domain = btn.getAttribute('data-domain') || 'this site';
      var msg = 'Delete site “‘ + domain + '”?\n\n' +
        'This will permanently:\n' +
        '• Delete all files and folders for this site (e.g. /var/www/' + domain + ')\n' +
        '• Remove the Caddy/FrankenPHP config for this site\n' +
        '• Delete all databases and database users associated with this site\n\n' +
        'This cannot be undone.';
      if (!confirm(msg)) return;
      var form = document.createElement('form');
      form.method = 'POST';
      form.action = '/sites/' + id + '/delete';
      document.body.appendChild(form);
      form.submit();
    });
  });
})();
