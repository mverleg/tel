document.addEventListener('DOMContentLoaded', function () {
  var main = document.querySelector('main');
  if (!main) return;
  if (main.querySelector('.tel-footer')) return;
  var footer = document.createElement('footer');
  footer.className = 'tel-footer';
  footer.innerHTML =
    'Source code: <a href="https://github.com/mverleg/tel" ' +
    'target="_blank" rel="noopener noreferrer">github.com/mverleg/tel</a>';
  main.appendChild(footer);
});
