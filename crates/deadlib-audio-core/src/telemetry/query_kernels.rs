//! Differential probes against the production implementation.
#![allow(dead_code)]
use super::*;
mod probes {
    use super::*;
    use std::hint::black_box;
    mod original {
        use super::*;
        include!("../../tests/support/diagnostic_reference.rs");
    }

    fn fixture(count: usize, kind: &str) -> AudioTelemetryState {
        let state = AudioTelemetryState::new();
        state.publish_output_timing(48000, 1000, 2000, 256, 128, 64, 3000);
        for i in 0..count {
            let at = if kind == "zero" || (kind == "mixed" && i % 3 == 0) {
                0
            } else {
                10000 + i as u64
            };
            state.record_stutter_diag_event(
                StutterDiagAudioEventKind::CallbackGap,
                at,
                i as u64,
                OutputTimingQuality::Trusted,
            );
        }
        if kind == "stale" {
            for slot in &state.stutter_diag_events {
                slot.version.fetch_add(128, Ordering::Relaxed);
            }
        }
        if kind == "writing" {
            for slot in &state.stutter_diag_events {
                slot.version.fetch_or(1, Ordering::Relaxed);
            }
        }
        if kind == "invalid" {
            for slot in &state.stutter_diag_events {
                slot.kind.store(255, Ordering::Relaxed);
            }
        }
        state
    }

    fn fields(event: &StutterDiagAudioEvent) -> [u64; 10] {
        [
            event.at_host_nanos,
            event.kind as u64,
            event.value_ns,
            event.sample_rate_hz as u64,
            event.buffer_frames as u64,
            event.padding_frames as u64,
            event.queued_frames as u64,
            event.device_period_ns,
            event.estimated_output_delay_ns,
            event.timing_quality as u64,
        ]
    }

    #[test]
    fn collection_keeps_correlated_payloads_during_ring_wraps() {
        let state = fixture(64, "live");
        let done = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let writer = scope.spawn(|| {
                for value in 64..100_064 {
                    state.record_stutter_diag_event(
                        StutterDiagAudioEventKind::CallbackGap,
                        10000 + value,
                        value,
                        OutputTimingQuality::Trusted,
                    );
                }
                done.store(true, Ordering::Release);
            });
            let mut output = Vec::with_capacity(64);
            while !done.load(Ordering::Acquire) {
                output.clear();
                state.collect_stutter_diag_events(u64::MAX, u64::MAX, &mut output);
                for event in &output {
                    assert_eq!(event.at_host_nanos, 10000 + event.value_ns);
                    assert_eq!(event.sample_rate_hz, 48000);
                    assert_eq!(event.kind, StutterDiagAudioEventKind::CallbackGap);
                }
                assert!(
                    output
                        .windows(2)
                        .all(|pair| pair[0].value_ns < pair[1].value_ns)
                );
            }
            writer.join().unwrap();
            let mut expected = Vec::new();
            output.clear();
            state.collect_stutter_diag_events(u64::MAX, u64::MAX, &mut output);
            state.collect_stutter_diag_events_reference(u64::MAX, u64::MAX, &mut expected);
            assert_eq!(output.len(), 64);
            assert_eq!(
                output.iter().map(fields).collect::<Vec<_>>(),
                expected.iter().map(fields).collect::<Vec<_>>()
            );
        });
    }

    #[test]
    fn collection_preserves_append_order_versions_and_time_boundaries() {
        for count in [0, 1, 63, 64, 65, 127, 255] {
            for kind in [
                "live", "expired", "zero", "stale", "mixed", "future", "writing", "invalid",
            ] {
                let state = fixture(count, kind);
                for now in [0, 5000, 10000, 10064, 1_000_000, u64::MAX] {
                    for window in [0, 1, 64, 1024, u64::MAX] {
                        let mut old = Vec::new();
                        let mut new = Vec::new();
                        state.collect_stutter_diag_events_reference(now, window, &mut old);
                        state.collect_stutter_diag_events(now, window, &mut new);
                        assert_eq!(
                            old.iter().map(fields).collect::<Vec<_>>(),
                            new.iter().map(fields).collect::<Vec<_>>()
                        );
                        let prefix = new.clone();
                        state.collect_stutter_diag_events(now, window, &mut new);
                        assert_eq!(
                            new.iter().map(fields).collect::<Vec<_>>(),
                            prefix.iter().chain(&prefix).map(fields).collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
        for head in [(1u64 << 63) - 1, 1 << 63, (1 << 63) + 63, u64::MAX] {
            let state = AudioTelemetryState::new();
            state.stutter_diag_event_head.store(head, Ordering::Relaxed);
            for seq in head - 63..=head {
                let slot =
                    &state.stutter_diag_events[(seq as usize - 1) % AUDIO_STUTTER_DIAG_EVENT_COUNT];
                slot.version.store(seq << 1, Ordering::Relaxed);
                slot.at_host_nanos.store(1, Ordering::Relaxed);
                slot.kind.store(1, Ordering::Relaxed);
            }
            let (mut old, mut new) = (Vec::new(), Vec::new());
            state.collect_stutter_diag_events_reference(1, 1, &mut old);
            state.collect_stutter_diag_events(1, 1, &mut new);
            assert_eq!(
                old.iter().map(fields).collect::<Vec<_>>(),
                new.iter().map(fields).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    #[ignore = "release CPU-cycle and allocation benchmark"]
    fn benchmark_query_diagnostics() {
        for count in [0, 1, 8, 64] {
            for kind in ["live", "expired", "zero", "stale", "mixed", "future"] {
                let state = fixture(count, kind);
                let now = match kind {
                    "expired" => 1_000_000,
                    "future" => 5000,
                    _ => 11000,
                };
                let mut output = Vec::with_capacity(64);
                let label = format!("query/diagnostics/{kind}/events={count}");
                let mut work = || {
                    output.clear();
                    state.collect_stutter_diag_events(
                        black_box(now),
                        black_box(1024),
                        black_box(&mut output),
                    );
                    black_box(&output);
                };
                let (_, counts) = crate::render::callback_kernels::perf::measure(&mut work);
                assert_eq!(
                    counts,
                    crate::render::callback_kernels::perf::Churn::default()
                );
                println!("{label}: allocation {counts:?}");
                crate::render::callback_kernels::bench::measure(&label, 20_000, &mut work);
            }
        }
    }
}
