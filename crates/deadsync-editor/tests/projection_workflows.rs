use deadsync_editor::projection::RowProjection;

#[test]
fn chart_elements_and_waveform_share_inverse_projection_at_every_zoom() {
    for scale in [0.000001, 0.02, 0.5, 1.0, 1.2, 16.0, 1000000.0] {
        let p = RowProjection::new(192, 160.0, scale, 105.0, 635.0).unwrap();
        assert_eq!(p.y(192), 160.0);
        for row in [0, 48, 191, 192, 193, 384, 100000] {
            let y = p.y(row);
            assert!((p.row_at(y) - row as f64).abs() < 0.000001);
            assert_eq!(p.clipped_y(row), y.clamp(105.0, 635.0));
        }
        let (a, b) = p.visible_rows();
        assert!(p.y(a) <= 105.0 || a == 0);
        assert!(p.y(b) >= 635.0);
        assert!((p.row_at(161.0) - p.row_at(160.0) - 1.0 / scale).abs() < 0.000001);
        assert_eq!(
            p.clipped_fractional_y(192.5),
            p.fractional_y(192.5).clamp(105.0, 635.0)
        );
    }
}

#[test]
fn integer_projection_preserves_neighboring_rows_at_large_anchors() {
    let p = RowProjection::new(u64::MAX - 1, 160.0, 2.0, 105.0, 635.0).unwrap();
    assert_eq!(p.y(u64::MAX), 162.0);
    assert_eq!(p.y(u64::MAX - 2), 158.0);
    assert_eq!(p.visible_rows(), (u64::MAX - 29, u64::MAX));
    let p = RowProjection::new(0, 160.0, 1.0, 105.0, 635.0).unwrap();
    assert_eq!(p.row_at(105.0), -55.0);
    assert_eq!(p.visible_rows(), (0, 475));
}

#[test]
fn invalid_projection_is_rejected() {
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(RowProjection::new(0, 160.0, scale, 105.0, 635.0).is_err());
    }
    for (cursor, top, bottom) in [
        (0.0, 1.0, 2.0),
        (3.0, 1.0, 2.0),
        (1.0, 2.0, 0.0),
        (f64::NAN, 0.0, 2.0),
    ] {
        assert!(RowProjection::new(0, cursor, 1.0, top, bottom).is_err());
    }
}
