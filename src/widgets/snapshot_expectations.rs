//! Expected semantic snapshots for widget SSR tests.

pub(super) const CHARTS: &str = r#"
node div.snapshot class="snapshot"
node div.snapshot > figure.pk-widget pk-chart pk-time-series class="pk-widget pk-chart pk-time-series"
node div.snapshot > figure.pk-widget pk-chart pk-time-series > figcaption
text div.snapshot > figure.pk-widget pk-chart pk-time-series > figcaption => Time series (ms)
node div.snapshot > figure.pk-widget pk-chart pk-time-series > div.pk-chart-plot class="pk-chart-plot" role="img" aria-label="time series chart: 1 series, unit ms"
node div.snapshot > figure.pk-widget pk-chart pk-time-series > div.pk-chart-plot > span.pk-chart-scale pk-chart-scale-max class="pk-chart-scale pk-chart-scale-max"
text div.snapshot > figure.pk-widget pk-chart pk-time-series > div.pk-chart-plot > span.pk-chart-scale pk-chart-scale-max => 3.50 ms
node div.snapshot > figure.pk-widget pk-chart pk-time-series > div.pk-chart-plot > span.pk-chart-scale pk-chart-scale-min class="pk-chart-scale pk-chart-scale-min"
text div.snapshot > figure.pk-widget pk-chart pk-time-series > div.pk-chart-plot > span.pk-chart-scale pk-chart-scale-min => 1 ms
node div.snapshot > figure.pk-widget pk-chart pk-time-series > div.pk-chart-plot > svg.pk-chart-svg class="pk-chart-svg" viewBox="0 0 320 140" preserveAspectRatio="none"
node div.snapshot > figure.pk-widget pk-chart pk-time-series > div.pk-chart-plot > svg.pk-chart-svg > polyline.pk-chart-line class="pk-chart-line" points="0.00,140.00 160.00,0.00 320.00,84.00" fill="none" stroke="var(--fg)" stroke-width="1.5" vector-effect="non-scaling-stroke"
node div.snapshot > figure.pk-widget pk-chart pk-time-series > div.pk-chart-xaxis class="pk-chart-xaxis" aria-hidden="true"
node div.snapshot > figure.pk-widget pk-chart pk-time-series > div.pk-chart-xaxis > span
text div.snapshot > figure.pk-widget pk-chart pk-time-series > div.pk-chart-xaxis > span => 0
node div.snapshot > figure.pk-widget pk-chart pk-time-series > div.pk-chart-xaxis > span
text div.snapshot > figure.pk-widget pk-chart pk-time-series > div.pk-chart-xaxis > span => 2
node div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-chart-legend class="pk-chart-legend" role="list" aria-label="series legend"
node div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-chart-legend > li.pk-chart-legend-item class="pk-chart-legend-item" aria-label="series latency"
node div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-chart-legend > li.pk-chart-legend-item > span.pk-chart-swatch class="pk-chart-swatch" aria-hidden="true" style="background:var(--fg);"
node div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-chart-legend > li.pk-chart-legend-item > span.pk-chart-legend-name class="pk-chart-legend-name"
text div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-chart-legend > li.pk-chart-legend-item > span.pk-chart-legend-name => latency
node div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-chart-data pk-visually-hidden class="pk-chart-data pk-visually-hidden"
node div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-chart-data pk-visually-hidden > li
text div.snapshot > figure.pk-widget pk-chart pk-time-series > ul.pk-chart-data pk-visually-hidden > li => series latency: 0:1 1:3.5 2:2
node div.snapshot > section.pk-widget pk-gauges class="pk-widget pk-gauges" role="group" aria-label="gauges"
node div.snapshot > section.pk-widget pk-gauges > div.pk-gauge class="pk-gauge" role="progressbar" aria-label="gauge queue 38%" aria-valuemin="0" aria-valuemax="100" aria-valuenow="38"
node div.snapshot > section.pk-widget pk-gauges > div.pk-gauge > span.pk-gauge-label class="pk-gauge-label"
text div.snapshot > section.pk-widget pk-gauges > div.pk-gauge > span.pk-gauge-label => queue
node div.snapshot > section.pk-widget pk-gauges > div.pk-gauge > span.pk-gauge-track class="pk-gauge-track"
node div.snapshot > section.pk-widget pk-gauges > div.pk-gauge > span.pk-gauge-track > span.pk-gauge-fill class="pk-gauge-fill" style="width:38%;"
node div.snapshot > section.pk-widget pk-gauges > div.pk-gauge > span.pk-gauge-value class="pk-gauge-value"
text div.snapshot > section.pk-widget pk-gauges > div.pk-gauge > span.pk-gauge-value => 3/8
node div.snapshot > section.pk-widget pk-flamegraph class="pk-widget pk-flamegraph" role="tree" aria-label="flamegraph"
node div.snapshot > section.pk-widget pk-flamegraph > div.pk-flame-frame class="pk-flame-frame" role="treeitem" data-depth="0" aria-label="root depth 0 value 8" style="--widget-c:rgb(10,20,30);--flame-depth:0;--flame-w:100.0%;"
node div.snapshot > section.pk-widget pk-flamegraph > div.pk-flame-frame > span.pk-flame-label class="pk-flame-label"
text div.snapshot > section.pk-widget pk-flamegraph > div.pk-flame-frame > span.pk-flame-label => root
node div.snapshot > section.pk-widget pk-flamegraph > div.pk-flame-frame > span.pk-flame-value class="pk-flame-value"
text div.snapshot > section.pk-widget pk-flamegraph > div.pk-flame-frame > span.pk-flame-value => 8
node div.snapshot > figure.pk-widget pk-chart pk-boxplot class="pk-widget pk-chart pk-boxplot"
node div.snapshot > figure.pk-widget pk-chart pk-boxplot > figcaption
text div.snapshot > figure.pk-widget pk-chart pk-boxplot > figcaption => Distribution
node div.snapshot > figure.pk-widget pk-chart pk-boxplot > ul.pk-box-list class="pk-box-list" role="list" aria-label="boxplot chart: 1 distributions"
node div.snapshot > figure.pk-widget pk-chart pk-boxplot > ul.pk-box-list > li.pk-box-item class="pk-box-item" style="--box-min:0.000%;--box-q1:25.000%;--box-median:50.000%;--box-q3:75.000%;--box-max:100.000%;--box-c:var(--fg);"
node div.snapshot > figure.pk-widget pk-chart pk-boxplot > ul.pk-box-list > li.pk-box-item > span.pk-box-label class="pk-box-label"
text div.snapshot > figure.pk-widget pk-chart pk-boxplot > ul.pk-box-list > li.pk-box-item > span.pk-box-label => p95
node div.snapshot > figure.pk-widget pk-chart pk-boxplot > ul.pk-box-list > li.pk-box-item > span.pk-box-track class="pk-box-track" role="img" aria-label="box plot p95: min 1, q1 2, median 3, q3 4, max 5"
node div.snapshot > figure.pk-widget pk-chart pk-boxplot > ul.pk-box-list > li.pk-box-item > span.pk-box-track > span.pk-box-whisker class="pk-box-whisker" aria-hidden="true"
node div.snapshot > figure.pk-widget pk-chart pk-boxplot > ul.pk-box-list > li.pk-box-item > span.pk-box-track > span.pk-box-range class="pk-box-range" aria-hidden="true"
node div.snapshot > figure.pk-widget pk-chart pk-boxplot > ul.pk-box-list > li.pk-box-item > span.pk-box-track > span.pk-box-median class="pk-box-median" aria-hidden="true"
node div.snapshot > figure.pk-widget pk-chart pk-boxplot > ul.pk-box-list > li.pk-box-item > span.pk-box-summary class="pk-box-summary" aria-hidden="true"
text div.snapshot > figure.pk-widget pk-chart pk-boxplot > ul.pk-box-list > li.pk-box-item > span.pk-box-summary => min 1, q1 2, median 3, q3 4, max 5
"#;

pub(super) const SCROLL_SPINNER: &str = r#"
node div.snapshot class="snapshot"
node div.snapshot > div.pk-widget pk-scroll pk-scroll-clip class="pk-widget pk-scroll pk-scroll-clip" role="region" aria-label="scrollable text" data-scroll-policy="clip" style="overflow:hidden;white-space:pre;"
node div.snapshot > div.pk-widget pk-scroll pk-scroll-clip > pre.pk-scroll-text class="pk-scroll-text"
text div.snapshot > div.pk-widget pk-scroll pk-scroll-clip > pre.pk-scroll-text => one two
node div.snapshot > div.pk-widget pk-scroll pk-scroll-wrap class="pk-widget pk-scroll pk-scroll-wrap" role="region" aria-label="scrollable text" data-scroll-policy="wrap" style="overflow-y:auto;overflow-x:hidden;white-space:pre-wrap;overflow-wrap:anywhere;"
node div.snapshot > div.pk-widget pk-scroll pk-scroll-wrap > pre.pk-scroll-text class="pk-scroll-text"
text div.snapshot > div.pk-widget pk-scroll pk-scroll-wrap > pre.pk-scroll-text => alpha beta
node div.snapshot > div.pk-widget pk-scroll pk-scroll-auto class="pk-widget pk-scroll pk-scroll-auto" role="region" aria-label="scrollable text" data-scroll-policy="auto" style="overflow-y:auto;overflow-x:hidden;white-space:pre-wrap;overflow-wrap:anywhere;"
node div.snapshot > div.pk-widget pk-scroll pk-scroll-auto > pre.pk-scroll-text class="pk-scroll-text"
text div.snapshot > div.pk-widget pk-scroll pk-scroll-auto > pre.pk-scroll-text => auto wrapped
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
