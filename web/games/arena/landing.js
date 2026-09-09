// Killshot landing page behaviour. Deliberately small: the page is readable and
// complete with this file absent or blocked, so nothing here may be load-bearing.
(function () {
  'use strict';

  // Reveal on scroll. The .reveal class is added HERE rather than in the markup
  // so a browser without JavaScript never ends up with a page of invisible
  // sections — the CSS hides only what this script can also un-hide.
  var targets = document.querySelectorAll('.band-head, .grid > article, .trailer, .endcta');
  if (!('IntersectionObserver' in window) || !targets.length) return;

  var reduced = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  if (reduced) return;

  Array.prototype.forEach.call(targets, function (el) { el.classList.add('reveal'); });

  var seen = new IntersectionObserver(function (entries) {
    entries.forEach(function (entry) {
      if (!entry.isIntersecting) return;
      entry.target.classList.add('in');
      seen.unobserve(entry.target);
    });
  }, { rootMargin: '0px 0px -8% 0px', threshold: 0.08 });

  Array.prototype.forEach.call(targets, function (el) { seen.observe(el); });

  // Safety net. The reveal is decoration, but hiding is the default state, so
  // anything that stops the observer from firing — a prerender, a background
  // tab that never paints, a browser quirk — would leave the page blank rather
  // than un-animated. After two seconds, show everything regardless.
  window.setTimeout(function () {
    Array.prototype.forEach.call(targets, function (el) { el.classList.add('in'); });
  }, 2000);

  // Pause the trailer when it scrolls out of view: a video that keeps talking
  // from off-screen is the single most irritating thing a landing page can do.
  var video = document.getElementById('trailer-video');
  if (!video) return;
  new IntersectionObserver(function (entries) {
    entries.forEach(function (entry) {
      if (!entry.isIntersecting && !video.paused) video.pause();
    });
  }, { threshold: 0.15 }).observe(video);
})();
