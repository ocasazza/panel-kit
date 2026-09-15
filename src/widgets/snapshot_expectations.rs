//! Expected semantic snapshots for widget SSR tests.

pub(super) const CHARTS: &str = r#"
node div.snapshot class="snapshot"
node div.snapshot > figure.pk-widget pk-chart pk-time-series class="pk-widget pk-chart pk-time-series" role="img" aria-label="time series chart: 1 series, unit ms"
node div.snapshot > figure.pk-widget pk-chart pk-time-series > figcaption
text div.snapshot > figure.pk-widget pk-chart pk-time-series > figcaption => Time series (ms)
node div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-series-list class="pk-series-list"
node div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-series-list > li.pk-series class="pk-series" aria-label="series latency"
node div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-series-list > li.pk-series > span.pk-series-name class="pk-series-name"
text div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-series-list > li.pk-series > span.pk-series-name => latency
node div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-series-list > li.pk-series > span.pk-series-points class="pk-series-points"
text div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-series-list > li.pk-series > span.pk-series-points => 0:1 1:3.5 2:2
node div.snapshot > section.pk-widget pk-gauges class="pk-widget pk-gauges" role="group" aria-label="gauges"
node div.snapshot > section.pk-widget pk-gauges > div.pk-gauge class="pk-gauge" role="progressbar" aria-label="gauge queue 38%" aria-valuemin="0" aria-valuemax="100" aria-valuenow="38"
node div.snapshot > section.pk-widget pk-gauges > div.pk-gauge > span.pk-gauge-label class="pk-gauge-label"
text div.snapshot > section.pk-widget pk-gauges > div.pk-gauge > span.pk-gauge-label => queue
node div.snapshot > section.pk-widget pk-gauges > div.pk-gauge > span.pk-gauge-track class="pk-gauge-track"
node div.snapshot > section.pk-widget pk-gauges > div.pk-gauge > span.pk-gauge-track > span.pk-gauge-fill class="pk-gauge-fill" style="width:38%;"
node div.snapshot > section.pk-widget pk-gauges > div.pk-gauge > span.pk-gauge-value class="pk-gauge-value"
text div.snapshot > section.pk-widget pk-gauges > div.pk-gauge > span.pk-gauge-value => 3/8
node div.snapshot > section.pk-widget pk-flamegraph class="pk-widget pk-flamegraph" role="tree" aria-label="flamegraph"
node div.snapshot > section.pk-widget pk-flamegraph > div.pk-flame-frame class="pk-flame-frame" role="treeitem" data-depth="0" aria-label="root depth 0 value 8" style="--widget-c:rgb(10,20,30);"
node div.snapshot > section.pk-widget pk-flamegraph > div.pk-flame-frame > span.pk-flame-label class="pk-flame-label"
text div.snapshot > section.pk-widget pk-flamegraph > div.pk-flame-frame > span.pk-flame-label => root
node div.snapshot > section.pk-widget pk-flamegraph > div.pk-flame-frame > span.pk-flame-value class="pk-flame-value"
text div.snapshot > section.pk-widget pk-flamegraph > div.pk-flame-frame > span.pk-flame-value => 8
node div.snapshot > figure.pk-widget pk-boxplot class="pk-widget pk-boxplot" role="img" aria-label="boxplot chart: 1 distributions"
node div.snapshot > figure.pk-widget pk-boxplot > figcaption
text div.snapshot > figure.pk-widget pk-boxplot > figcaption => Distribution
node div.snapshot > figure.pk-widget pk-boxplot > ul.pk-box-list class="pk-box-list"
node div.snapshot > figure.pk-widget pk-boxplot > ul.pk-box-list > li.pk-box-item class="pk-box-item" style=""
node div.snapshot > figure.pk-widget pk-boxplot > ul.pk-box-list > li.pk-box-item > span.pk-box-label class="pk-box-label"
text div.snapshot > figure.pk-widget pk-boxplot > ul.pk-box-list > li.pk-box-item > span.pk-box-label => p95
node div.snapshot > figure.pk-widget pk-boxplot > ul.pk-box-list > li.pk-box-item > span.pk-box-summary class="pk-box-summary"
text div.snapshot > figure.pk-widget pk-boxplot > ul.pk-box-list > li.pk-box-item > span.pk-box-summary => min 1, q1 2, median 3, q3 4, max 5
"#;

pub(super) const TABLE_METER_STATUS: &str = r#"
node div.snapshot class="snapshot"
node div.snapshot > table.pk-widget pk-table class="pk-widget pk-table"
node div.snapshot > table.pk-widget pk-table > thead
node div.snapshot > table.pk-widget pk-table > thead > tr
node div.snapshot > table.pk-widget pk-table > thead > tr > th scope="col" style="width:2fr;text-align:left;"
text div.snapshot > table.pk-widget pk-table > thead > tr > th => Service
node div.snapshot > table.pk-widget pk-table > thead > tr > th scope="col" style="width:8ch;text-align:center;"
text div.snapshot > table.pk-widget pk-table > thead > tr > th => Health
node div.snapshot > table.pk-widget pk-table > thead > tr > th scope="col" style="width:10ch;text-align:right;"
text div.snapshot > table.pk-widget pk-table > thead > tr > th => Load
node div.snapshot > table.pk-widget pk-table > tbody
node div.snapshot > table.pk-widget pk-table > tbody > tr
node div.snapshot > table.pk-widget pk-table > tbody > tr > td style="text-align:left;"
text div.snapshot > table.pk-widget pk-table > tbody > tr > td => api
node div.snapshot > table.pk-widget pk-table > tbody > tr > td style="text-align:center;--status-c:rgb(39,201,63);"
node div.snapshot > table.pk-widget pk-table > tbody > tr > td > span.pk-table-status class="pk-table-status" role="status" aria-label="ok"
node div.snapshot > table.pk-widget pk-table > tbody > tr > td > span.pk-table-status > span aria-hidden="true"
text div.snapshot > table.pk-widget pk-table > tbody > tr > td > span.pk-table-status > span => ●
text div.snapshot > table.pk-widget pk-table > tbody > tr > td > span.pk-table-status => ok
node div.snapshot > table.pk-widget pk-table > tbody > tr > td style="text-align:right;--meter-c:rgb(94,243,140);"
node div.snapshot > table.pk-widget pk-table > tbody > tr > td > span.pk-table-meter class="pk-table-meter" role="meter" aria-valuemin="0" aria-valuemax="100" aria-valuenow="42" aria-label="42%"
node div.snapshot > table.pk-widget pk-table > tbody > tr > td > span.pk-table-meter > span.pk-table-meter-fill class="pk-table-meter-fill" style="width:42%;"
node div.snapshot > table.pk-widget pk-table > tbody > tr > td > span.pk-table-meter > span.pk-table-meter-text class="pk-table-meter-text"
text div.snapshot > table.pk-widget pk-table > tbody > tr > td > span.pk-table-meter > span.pk-table-meter-text => 42%
node div.snapshot > div.pk-widget pk-meter class="pk-widget pk-meter" role="progressbar" aria-label="meter memory 63%" aria-valuemin="0" aria-valuemax="100" aria-valuenow="63" style="--meter-c:rgb(94,243,140);"
node div.snapshot > div.pk-widget pk-meter > span.pk-meter-label class="pk-meter-label"
text div.snapshot > div.pk-widget pk-meter > span.pk-meter-label => memory
node div.snapshot > div.pk-widget pk-meter > span.pk-meter-track class="pk-meter-track"
node div.snapshot > div.pk-widget pk-meter > span.pk-meter-track > span.pk-meter-fill class="pk-meter-fill" style="width:63%;"
node div.snapshot > div.pk-widget pk-meter > span.pk-meter-value class="pk-meter-value"
text div.snapshot > div.pk-widget pk-meter > span.pk-meter-value => 5 GiB / 8 GiB
node div.snapshot > span.pk-widget pk-status pk-status-warning class="pk-widget pk-status pk-status-warning" role="status" aria-label="deploy warning" style="--status-c:rgb(255,189,46);"
node div.snapshot > span.pk-widget pk-status pk-status-warning > span.pk-status-dot class="pk-status-dot" aria-hidden="true"
text div.snapshot > span.pk-widget pk-status pk-status-warning > span.pk-status-dot => ●
node div.snapshot > span.pk-widget pk-status pk-status-warning > span.pk-status-label class="pk-status-label"
text div.snapshot > span.pk-widget pk-status pk-status-warning > span.pk-status-label => deploy
"#;

pub(super) const SCROLL_SPINNER: &str = r#"
node div.snapshot class="snapshot"
node div.snapshot > div.pk-widget pk-scroll pk-scroll-clip class="pk-widget pk-scroll pk-scroll-clip" role="region" aria-label="scrollable text" data-scroll-policy="clip" style="overflow:hidden;white-space:pre;"
node div.snapshot > div.pk-widget pk-scroll pk-scroll-clip > pre.pk-scroll-text class="pk-scroll-text"
text div.snapshot > div.pk-widget pk-scroll pk-scroll-clip > pre.pk-scroll-text => one two
node div.snapshot > div.pk-widget pk-scroll pk-scroll-wrap class="pk-widget pk-scroll pk-scroll-wrap" role="region" aria-label="scrollable text" data-scroll-policy="wrap" style="overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere;"
node div.snapshot > div.pk-widget pk-scroll pk-scroll-wrap > pre.pk-scroll-text class="pk-scroll-text"
text div.snapshot > div.pk-widget pk-scroll pk-scroll-wrap > pre.pk-scroll-text => alpha beta
node div.snapshot > span.pk-widget spinner class="pk-widget spinner" role="status" aria-live="polite" aria-label="indexing"
node div.snapshot > span.pk-widget spinner > span.spin-ring class="spin-ring" aria-hidden="true"
text div.snapshot > span.pk-widget spinner > span.spin-ring => ⠋
node div.snapshot > span.pk-widget spinner > span.spin-label class="spin-label"
text div.snapshot > span.pk-widget spinner > span.spin-label => indexing
node div.snapshot > span.pk-widget spinner class="pk-widget spinner" role="status" aria-live="polite" aria-label="syncing"
node div.snapshot > span.pk-widget spinner > span.spin-ring class="spin-ring" aria-hidden="true"
text div.snapshot > span.pk-widget spinner > span.spin-ring => ⠋
node div.snapshot > span.pk-widget spinner > span.spin-label class="spin-label"
text div.snapshot > span.pk-widget spinner > span.spin-label => syncing
"#;

pub(super) const BADGE_STRIP: &str = r#"
node div.pk-widget pk-badge-strip class="pk-widget pk-badge-strip" role="list" aria-label="badges"
node div.pk-widget pk-badge-strip > span.pk-badge-item class="pk-badge-item" role="listitem"
node div.pk-widget pk-badge-strip > span.pk-badge-item > span.badge badge-tag active class="badge badge-tag active" style="" role="group" title="alpha"
node div.pk-widget pk-badge-strip > span.pk-badge-item > span.badge badge-tag active > button.badge-main class="badge-main" type="button" aria-label="badge:tag=alpha"
node div.pk-widget pk-badge-strip > span.pk-badge-item > span.badge badge-tag active > button.badge-main > span.badge-label class="badge-label"
text div.pk-widget pk-badge-strip > span.pk-badge-item > span.badge badge-tag active > button.badge-main > span.badge-label => alpha
node div.pk-widget pk-badge-strip > span.pk-badge-item > span.badge badge-tag active > button.badge-btn badge-plus class="badge-btn badge-plus" type="button" aria-label="Add filter: tag=alpha"
text div.pk-widget pk-badge-strip > span.pk-badge-item > span.badge badge-tag active > button.badge-btn badge-plus => +
node div.pk-widget pk-badge-strip > span.pk-badge-item > span.badge badge-tag active > button.badge-btn badge-x class="badge-btn badge-x" type="button" aria-label="Toggle filter: tag=alpha"
text div.pk-widget pk-badge-strip > span.pk-badge-item > span.badge badge-tag active > button.badge-btn badge-x => ×
node div.pk-widget pk-badge-strip > span.pk-badge-item class="pk-badge-item" role="listitem"
node div.pk-widget pk-badge-strip > span.pk-badge-item > span.badge badge-wikilink class="badge badge-wikilink" style="" role="group" title="Panel Kit"
node div.pk-widget pk-badge-strip > span.pk-badge-item > span.badge badge-wikilink > button.badge-main class="badge-main" type="button" aria-label="badge:link=Panel Kit"
node div.pk-widget pk-badge-strip > span.pk-badge-item > span.badge badge-wikilink > button.badge-main > span.badge-label class="badge-label"
text div.pk-widget pk-badge-strip > span.pk-badge-item > span.badge badge-wikilink > button.badge-main > span.badge-label => ⟶ Panel Kit
"#;
