var app = import('../pkg/counter.js');

(function () {
    'use strict';
    app.then(app => app.default());
})();
