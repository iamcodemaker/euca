var app = import('../pkg/todomvc.js');

(function () {
    'use strict';
    app.then(app => app.default());
})();
