document.addEventListener('DOMContentLoaded', function () {
  var main = document.querySelector('main');
  if (!main) return;
  if (main.querySelector('.tel-banner')) return;
  var banner = document.createElement('div');
  banner.className = 'tel-banner';
  banner.setAttribute('role', 'note');
  banner.innerHTML =
    '\u{1F6A7} <strong>Under construction.</strong> This documentation ' +
    'describes a language still being designed and is subject to change.';
  main.insertBefore(banner, main.firstChild);
});
