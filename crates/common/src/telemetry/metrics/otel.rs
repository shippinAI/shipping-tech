use crate::config::telemetry::OtelMetrics;
use ahash::AHashMap;
use opentelemetry::metrics::MeterProvider;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::Resource;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
use trc::Collector;

impl OtelMetrics {
    pub fn new(
        exporter: opentelemetry_otlp::MetricExporter,
        resource: Resource,
        interval: Duration,
    ) -> Self {
        let reader = PeriodicReader::builder(exporter)
            .with_interval(Duration::from_secs(60 * 60 * 24))
            .build();
        let provider = SdkMeterProvider::builder()
            .with_resource(resource)
            .with_reader(reader)
            .build();

        Self {
            provider,
            interval,
            last_counters: Mutex::new(AHashMap::new()),
            last_histograms: Mutex::new(AHashMap::new()),
        }
    }

    pub async fn push_metrics(&self, is_enterprise: bool, _start_time: SystemTime) {
        let meter = self.provider.meter("shippin.tech");

        {
            let mut last = self.last_counters.lock().unwrap_or_else(|err| err.into_inner());
            for counter in Collector::collect_counters(is_enterprise) {
                let id = counter.id();
                let value = counter.value();
                let prev = last.get(&id.to_id()).copied().unwrap_or(0);
                if value > prev {
                    meter
                        .u64_counter(id.as_str())
                        .with_description(id.description())
                        .with_unit("events")
                        .build()
                        .add(value - prev, &[] as &[KeyValue]);
                }
                last.insert(id.to_id(), value);
            }
        }

        for gauge in Collector::collect_gauges(is_enterprise) {
            let id = gauge.id();
            meter
                .u64_gauge(id.as_str())
                .with_description(id.description())
                .with_unit(id.unit())
                .build()
                .record(gauge.get(), &[] as &[KeyValue]);
        }

        {
            let mut last = self
                .last_histograms
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            for histogram in Collector::collect_histograms(is_enterprise) {
                let id = histogram.id();
                let buckets = histogram.buckets_vec();
                let bounds = histogram.upper_bounds_vec();
                let prev = last.entry(id.to_id()).or_default();
                if prev.len() != buckets.len() {
                    prev.resize(buckets.len(), 0);
                }
                let instrument = meter
                    .u64_histogram(id.as_str())
                    .with_description(id.description())
                    .with_unit(id.unit())
                    .build();
                for (index, count) in buckets.iter().copied().enumerate() {
                    let seen = prev[index];
                    if count > seen {
                        let bound = bounds.get(index).copied().unwrap_or_else(|| {
                            bounds.last().copied().unwrap_or(0.0)
                        }) as u64;
                        for _ in 0..(count - seen) {
                            instrument.record(bound, &[] as &[KeyValue]);
                        }
                    }
                    prev[index] = count;
                }
            }
        }

        if let Err(err) = self.provider.force_flush() {
            trc::event!(
                Telemetry(trc::TelemetryEvent::OtelMetricsExporterError),
                Details = "Failed to export metrics",
                Reason = err.to_string()
            );
        }
    }

    pub fn enable_errors() {
        // The published OpenTelemetry SDK has no set_error_handler hook.
    }
}
